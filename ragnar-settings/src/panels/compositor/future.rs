// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — compositor panel: future surface
// Copyright (C) 2026 LieraOS Project
//
// Depth the running compositor cannot provide — listed locked so
// the surface is already designed. Each row asks the capability
// map: once the native Ragnar compositor runs, `supports` flips
// and the row's real control replaces the lock note.

use super::widgets::locked_row;
use crate::backend::compositor::Cap;
use crate::i18n::msgid;
use crate::panels::common;
use gettextrs::gettext;
use gtk4::prelude::*;

/// Locked features: (title, capability).
const LOCKED: &[(&str, Cap)] = &[
    (msgid("Backdrop blur"), Cap::Blur),
    (msgid("Window animations"), Cap::Animations),
    (msgid("Tiling layouts"), Cap::Layouts),
    (msgid("Touchpad workspace gestures"), Cap::Gestures),
    (msgid("Window groups (tabs)"), Cap::WindowGroups),
    (msgid("Scratchpad workspace"), Cap::Scratchpad),
    (msgid("Monitor mirroring"), Cap::Mirror),
];

/// The "requires the Ragnar compositor" card.
pub fn section() -> gtk4::Box {
    let b = common::section(&gettext("Coming with the native compositor"));
    let card = common::card();
    for (title, cap) in LOCKED {
        locked_row(&card, &gettext(*title), *cap);
    }
    b.append(&card);
    b.append(&common::note(&gettext(
        "These controls are already placed so nothing needs to be \
         re-arranged later. They unlock automatically as soon as \
         the compositor provides them.",
    )));
    b
}
