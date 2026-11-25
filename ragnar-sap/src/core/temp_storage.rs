// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — temporary storage,.
//
// Scan data lands on disk between pipeline stages (preview, print
// hand-off, sessions). The obfuscates files with a random XOR
// key stored in the handle, tracks expiry via a sibling.exp file
// holding epoch-nanos, secure-overwrites before delete and purges
// expired entries. Identical semantics here — random bytes come
// from /dev/urandom, no extra dependencies.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EXPIRY_SUFFIX: &str = ".exp";

/// Reference to one stored blob — caller keeps it to load/remove.
#[derive(Debug, Clone)]
pub struct TempHandle {
 /// File name relative to the storage() root.
 pub relative_path: String,
 /// XOR key as hex, applied to the stored bytes.
 pub key_hex: String,
 /// Expiry instant (unix epoch nanos).
 pub expires_at_nanos: i128,
}

/// Disk-backed temporary store with TTL + obfuscation.
pub struct TempStorage {
 root: PathBuf,
}

/// Read n random bytes via getrandom(2) — the same entropy source
/// std::random_device uses , without file reads.
fn random_bytes(n: usize) -> Vec<u8> {
 let mut out = vec![0u8; n];
 let mut filled = 0usize;
 unsafe {
 while filled < n {
 let rc = libc::getrandom(out.as_mut_ptr().add(filled) as *mut libc::c_void,
 n - filled,
 0,
 );
 if rc <= 0 {
 break;
 }
 filled += rc as usize;
 }
 }
 if filled == n {
 return out;
 }
 // Fallback only when getrandom is unavailable: time+pid mix.
 let t = SystemTime::now()
.duration_since(UNIX_EPOCH)
.map(|d| d.as_nanos())
.unwrap_or(0);
 let mut x = t ^ (std::process::id() as u128);
 while filled < n {
 x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
 for b in x.to_le_bytes() {
 if filled < n {
 out[filled] = b;
 filled += 1;
 }
 }
 }
 out
}

fn random_hex(n: usize) -> String {
 random_bytes(n / 2)
.iter()
.map(|b| format!("{:02x}", b))
.collect()
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
 (0..hex.len().saturating_sub(1))
.step_by(2)
.filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
.collect()
}

fn bytes_to_hex(data: &[u8]) -> String {
 data.iter().map(|b| format!("{:02x}", b)).collect()
}

fn apply_xor(data: &mut [u8], key: &[u8]) {
 if key.is_empty() {
 return;
 }
 for (i, b) in data.iter_mut().enumerate() {
 *b ^= key[i % key.len()];
 }
}

fn now_nanos() -> i128 {
 SystemTime::now()
.duration_since(UNIX_EPOCH)
.map(|d| d.as_nanos() as i128)
.unwrap_or(0)
}

impl TempStorage {
 /// Create/open the storage() root (e.g. /tmp/ragnar-sap).
 pub fn new(root: impl Into<PathBuf>) -> Self {
 let s = Self { root: root.into() };
 s.ensure_root();
 s
 }

 fn ensure_root(&self) {
 let _ = std::fs::create_dir_all(&self.root);
 }

 fn data_path(&self, rel: &str) -> PathBuf {
 self.root.join(rel)
 }

 fn expiry_path(&self, rel: &str) -> PathBuf {
 self.root.join(format!("{}{}", rel, EXPIRY_SUFFIX))
 }

 /// Store data with a TTL; returns the handle needed for
 /// load/remove — same contract.
 pub fn store(&self, data: &[u8], ttl: Duration) -> TempHandle {
 self.ensure_root();
 let key = random_bytes(16);
 let mut buf = data.to_vec();
 apply_xor(&mut buf, &key);
 let rel = format!("{}.bin", random_hex(24));
 let path = self.data_path(&rel);
 let _ = std::fs::write(&path, &buf);
 let expires = now_nanos() + ttl.as_nanos() as i128;
 let _ = std::fs::write(self.expiry_path(&rel), expires.to_string());
 TempHandle {
 relative_path: rel,
 key_hex: bytes_to_hex(&key),
 expires_at_nanos: expires,
 }
 }

 /// Load + de-obfuscate a stored blob.
 pub fn load(&self, handle: &TempHandle) -> Option<Vec<u8>> {
 let mut data = std::fs::read(self.data_path(&handle.relative_path)).ok()?;
 if data.is_empty() {
 return None;
 }
 apply_xor(&mut data, &hex_to_bytes(&handle.key_hex));
 Some(data)
 }

 /// Secure-overwrite then delete data + expiry sidecar.
 pub fn remove(&self, handle: &TempHandle) {
 let path = self.data_path(&handle.relative_path);
 Self::secure_overwrite(&path);
 let _ = std::fs::remove_file(&path);
 let _ = std::fs::remove_file(self.expiry_path(&handle.relative_path));
 }

 /// Delete every entry whose expiry metadata lies in the past.
 pub fn purge_expired(&self) {
 let now = now_nanos();
 let entries = match std::fs::read_dir(&self.root) {
 Ok(e) => e,
 Err(_) => return,
 };
 for entry in entries.flatten() {
 let p = entry.path();
 if p.extension().map(|e| e == "exp").unwrap_or(false) {
 if let Ok(txt) = std::fs::read_to_string(&p) {
 if let Ok(ticks) = txt.trim().parse::<i128>() {
 if ticks < now {
 let data_file = p.with_extension("");
 Self::secure_overwrite(&data_file);
 let _ = std::fs::remove_file(&data_file);
 let _ = std::fs::remove_file(&p);
 }
 }
 }
 }
 }
 }

 /// Replace file content with random bytes before unlinking.
 fn secure_overwrite(path: &Path) {
 if let Ok(meta) = std::fs::metadata(path) {
 if meta.len() > 0 {
 let noise = random_bytes(meta.len() as usize);
 let _ = std::fs::write(path, noise);
 }
 }
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use std::time::Duration;

 #[test]
 fn store_load_roundtrip() {
 let dir = std::env::temp_dir().join(format!("ragnar-sap-test-{}", std::process::id()));
 let store = TempStorage::new(&dir);
 let payload = b"hello scan data".to_vec();
 let handle = store.store(&payload, Duration::from_secs(3600));
 assert_eq!(store.load(&handle), Some(payload));
 // Stored bytes on disk must be obfuscated — not plaintext.
 let on_disk = std::fs::read(dir.join(&handle.relative_path)).unwrap();
 assert_ne!(on_disk, b"hello scan data".to_vec());
 store.remove(&handle);
 assert!(store.load(&handle).is_none());
 let _ = std::fs::remove_dir_all(&dir);
 }

 #[test]
 fn purge_removes_expired() {
 let dir = std::env::temp_dir().join(format!("ragnar-sap-test2-{}", std::process::id()));
 let store = TempStorage::new(&dir);
 let h = store.store(b"x", Duration::from_nanos(1));
 std::thread::sleep(Duration::from_millis(10));
 store.purge_expired();
 assert!(!dir.join(&h.relative_path).exists());
 let _ = std::fs::remove_dir_all(&dir);
 }
}
