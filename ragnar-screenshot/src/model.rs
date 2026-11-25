// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// Shared types between capture, overlay and UI.

/// What the user wants captured.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// All monitors merged into one picture.
    All,
    /// A single monitor, picked from the list.
    Monitor(usize),
    /// Free rectangle selection.
    Area,
    /// The window under the pointer when clicked.
    Window,
}

/// Persistent options from the main window.
#[derive(Clone)]
pub struct Options {
    pub mode: Mode,
    pub show_cursor: bool,
    pub play_sound: bool,
    pub delay_secs: u32,
}

/// One captured frame in plain RGBA bytes.
pub struct CapturedImage {
    pub width: usize,
    pub height: usize,
    /// RGBA, row-major, 4 bytes per pixel.
    pub pixels: Vec<u8>,
}
