// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — library surface.
//
// The binary is a thin shell; all decode/UI modules live here so
// integration tests can exercise them.

pub mod browser;
pub mod cache;
pub mod canvas;
pub mod decode;
pub mod filters;
pub mod i18n;
pub mod thumbs;
pub mod window;
