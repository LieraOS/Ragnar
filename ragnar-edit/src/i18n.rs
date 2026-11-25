//! Ragnar Edit
//!
//! Copyright (C) 2026 Jörg Schröder
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
//! General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program. If not, see <https://www.gnu.org/licenses/>.

//! gettext initialisation — domain "ragnar-edit".

use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};

/// Binds the gettext domain and forces UTF-8 output for all locales.
pub fn init() {
    setlocale(LocaleCategory::LcAll, "");
    bindtextdomain("ragnar-edit", "/usr/share/locale").expect("bindtextdomain");
    bind_textdomain_codeset("ragnar-edit", "UTF-8").expect("codeset");
    textdomain("ragnar-edit").expect("textdomain");
}
