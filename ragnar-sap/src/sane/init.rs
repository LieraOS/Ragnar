// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — process-wide libsane initialisation.
//
// sane_init must run before any other SANE call and sane_exit
// releases the backend state at shutdown. We init once behind a
// OnceLock so discovery and sessions can run from any thread.

use super::{status, sys};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

static INIT_RESULT: OnceLock<Result<i32, super::status::SaneError>> = OnceLock::new();
static EXITED: AtomicBool = AtomicBool::new(false);
/// libsane is not thread-safe — every FFI call must hold this lock.
static FFI_LOCK: Mutex<()> = Mutex::new(());

/// Serialize access to libsane for the duration of the guard.
/// Poisoned mutexes are recovered so a panicked caller cannot
/// wedge all later SANE access.
pub(crate) fn lock() -> MutexGuard<'static, ()> {
 FFI_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Ensure libsane is initialised. Returns the negotiated API version
/// or the init error. Subsequent calls reuse the cached outcome.
pub fn ensure_init() -> Result<i32, super::status::SaneError> {
 INIT_RESULT
.get_or_init(|| {
 // No FFI lock here — OnceLock serialises this itself and
 // callers already hold the guard (std::sync::Mutex is not
 // reentrant, a second lock() here would deadlock).
 let mut version: sys::SANE_Int = 0;
 let st = unsafe { sys::sane_init(&mut version, std::ptr::null_mut()) };
 status::check(st).map(|_| version)
 })
.clone()
}

/// Shut libsane down. Safe to call at process exit; subsequent SANE
/// calls after this point will fail their own status checks.
pub fn shutdown() {
 if !EXITED.swap(true, Ordering::SeqCst) {
 let _g = lock();
 unsafe { sys::sane_exit() };
 }
}
