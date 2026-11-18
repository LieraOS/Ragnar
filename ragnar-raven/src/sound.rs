//! freedesktop event sounds via libcanberra — linked directly.
//!
//! The Debian binary `canberra-gtk-play` lives in the package
//! `gnome-session-canberra` — a GNOME-named dependency Ragnar must
//! not carry, and spawning a process per notification is wasteful
//! anyway. `libcanberra0` is the small neutral freedesktop
//! sound-event library itself; we call its C API in-process and keep
//! one cached context for the whole daemon lifetime.

use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

/// Opaque libcanberra context handle — layout is private to the
/// library, we only ever hold the pointer it hands back.
#[repr(C)]
struct CaContext {
    _private: [u8; 0],
}

// Link the versioned soname directly (-l:libcanberra.so.0): the
// runtime package libcanberra0 ships it, so no -dev package is
// needed to build — while symbols are still checked at link time.
#[link(name = ":libcanberra.so.0")]
unsafe extern "C" {
    fn ca_context_create(c: *mut *mut CaContext) -> c_int;
    fn ca_context_open(c: *mut CaContext) -> c_int;
    /// C-variadic: property key/value pairs, NULL-terminated.
    fn ca_context_play(c: *mut CaContext, id: u32, ...) -> c_int;
}

thread_local! {
    /// Cached context — creating one per beep would defeat
    /// canberra's sample caching, and destroying it before the async
    /// playback finished would cut sounds short.
    static CTX: RefCell<Option<*mut CaContext>> = const { RefCell::new(None) };
}

/// Get or lazily create the cached context — one per process so
/// canberra's sample caching actually works and playback outlives
/// the call.
fn ctx() -> Option<*mut CaContext> {
    CTX.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let mut ctx = std::ptr::null_mut();
            unsafe {
                if ca_context_create(&mut ctx) != 0 || ctx.is_null() {
                    return None;
                }
                let _ = ca_context_open(ctx);
            }
            *slot = Some(ctx);
        }
        *slot
    })
}

/// Play a freedesktop sound-event id (e.g. `dialog-information`,
/// `bell`). Does nothing when canberra is unavailable — notification
/// sound is best-effort by design.
pub fn play_event(name: &str) {
    let Ok(name) = CString::new(name) else {
        return;
    };
    let Some(ctx) = ctx() else { return };
    unsafe {
        ca_context_play(
            ctx,
            0,
            c"event.id".as_ptr(),
            name.as_ptr(),
            c"media.role".as_ptr(),
            c"event".as_ptr(),
            std::ptr::null::<c_char>(),
        );
    }
}

/// Play an arbitrary audio file — the per-appointment custom sound
/// (`media.filename` instead of an event id). Missing/unreadable
/// files fail silently inside canberra — best-effort by design.
pub fn play_file(path: &std::path::Path) {
    let Ok(p) = CString::new(path.to_string_lossy().as_bytes()) else {
        return;
    };
    let Some(ctx) = ctx() else { return };
    unsafe {
        ca_context_play(
            ctx,
            0,
            c"media.filename".as_ptr(),
            p.as_ptr(),
            c"media.role".as_ptr(),
            c"event".as_ptr(),
            std::ptr::null::<c_char>(),
        );
    }
}
