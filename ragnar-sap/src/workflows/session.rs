// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — scan session manager(),.
//
// Sessions hold the current scan payload inside TempStorage; ids
// are 16-hex random strings. updateSession replaces the stored
// blob, finalize/abort destroy it. The guards sessions_ with a
// mutex — HashMap inside Mutex gives identical semantics.

use crate::core::temp_storage::{TempHandle, TempStorage};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One session id -> storage() handle.
type Sessions = HashMap<String, TempHandle>;

/// Shared manager() — Arc<TempStorage> matches shared_ptr.
pub struct ScanSessionManager {
 storage: Arc<TempStorage>,
 default_ttl: Duration,
 sessions: Mutex<Sessions>,
}

impl ScanSessionManager {
 /// Wrap a TempStorage with a default TTL.
 pub fn new(storage: Arc<TempStorage>, default_ttl: Duration) -> Self {
 Self {
 storage,
 default_ttl,
 sessions: Mutex::new(HashMap::new()),
 }
 }

 fn lock(&self) -> std::sync::MutexGuard<'_, Sessions> {
 self.sessions.lock().unwrap_or_else(|e| e.into_inner())
 }

 /// 16-hex session id via getrandom — same scheme 
 /// mt19937_64-generated hex ids.
 fn session_id() -> String {
 let mut buf = [0u8; 8];
 let n = unsafe {
 libc::getrandom(buf.as_mut_ptr() as *mut libc::c_void, 8, 0)
 };
 if n == 8 {
 buf.iter().map(|x| format!("{:02x}", x)).collect()
 } else {
 format!("{:016x}", std::process::id())
 }
 }

 /// Start a session with initial data; returns the new id.
 pub fn begin(&self, initial_data: &[u8]) -> String {
 let id = Self::session_id();
 let handle = self.storage.store(initial_data, self.default_ttl);
 self.lock().insert(id.clone(), handle);
 id
 }

 /// Replace a session's payload — drops the old blob first,
 /// identical to updateSession.
 pub fn update(&self, session_id: &str, data: &[u8]) {
 let mut sessions = self.lock();
 if let Some(old) = sessions.get(session_id) {
 self.storage.remove(old);
 }
 let handle = self.storage.store(data, self.default_ttl);
 sessions.insert(session_id.to_string(), handle);
 }

 /// Load a session's payload.
 pub fn load(&self, session_id: &str) -> Option<Vec<u8>> {
 let handle = self.lock().get(session_id)?.clone();
 self.storage.load(&handle)
 }

 /// Finalize: destroy the session + its stored blob.
 pub fn finalize(&self, session_id: &str) {
 self.destroy(session_id);
 }

 /// Abort: identical to finalize in — same destroy path.
 pub fn abort(&self, session_id: &str) {
 self.destroy(session_id);
 }

 fn destroy(&self, session_id: &str) {
 let handle = self.lock().remove(session_id);
 if let Some(h) = handle {
 self.storage.remove(&h);
 }
 }

 /// Drop expired storage() entries — purgeExpiredSessions.
 pub fn purge_expired(&self) {
 self.storage.purge_expired();
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::core::temp_storage::TempStorage;

 fn manager() -> Arc<ScanSessionManager> {
 let dir = std::env::temp_dir().join(format!("ragnar-sap-sess-{}", std::process::id()));
 Arc::new(ScanSessionManager::new(Arc::new(TempStorage::new(dir)),
 Duration::from_secs(3600),
 ))
 }

 #[test]
 fn begin_update_load_finalize() {
 let m = manager();
 let id = m.begin(b"v1");
 assert_eq!(m.load(&id), Some(b"v1".to_vec()));
 m.update(&id, b"v2");
 assert_eq!(m.load(&id), Some(b"v2".to_vec()));
 m.finalize(&id);
 assert_eq!(m.load(&id), None);
 }

 #[test]
 fn abort_drops_payload() {
 let m = manager();
 let id = m.begin(b"data");
 m.abort(&id);
 assert_eq!(m.load(&id), None);
 }
}
