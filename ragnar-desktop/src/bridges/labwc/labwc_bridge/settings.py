# This file is part of ragnar-desktop
#
# Copyright (C) Jörg Schröder, LieraOS Project
#
# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 2 of the License, or
# (at your option) any later version.

import logging

from gi.repository import Gio

log = logging.getLogger(__name__)


class Settings:
    """The gsettings schemas the bridge follows."""

    def __init__(self) -> None:
        # Gio aborts the process on a schema that is not installed, so each id is
        # looked up first; a schema that is absent leaves the attribute as None
        # and every consumer skips it.
        source = Gio.SettingsSchemaSource.get_default()

        def optional(schema_id: str) -> Gio.Settings | None:
            if source.lookup(schema_id, True) is None:
                log.warning("gsettings schema not installed: %s", schema_id)
                return None
            return Gio.Settings.new(schema_id)

        self.panel = optional("org.lieraos.ragnar.panel")
        self.media_keys = optional(
            "org.lieraos.ragnar.settings-daemon.plugins.media-keys"
        )
        self.wm_keybindings = optional("org.gnome.desktop.wm.keybindings")
        self.wm_preferences = optional("org.gnome.desktop.wm.preferences")
        self.mutter_keybindings = optional("org.gnome.mutter.keybindings")
        self.interface = optional("org.gnome.desktop.interface")
        self.mutter = optional("org.gnome.mutter")
        self.ragnar_wm = optional("org.lieraos.ragnar.wm")
        self.input_sources = optional("org.gnome.desktop.input-sources")
        self.mouse = optional("org.gnome.desktop.peripherals.mouse")
        self.touchpad = optional("org.gnome.desktop.peripherals.touchpad")

    def for_template(self, short_schema: str) -> Gio.Settings | None:
        """The schema a keybinds.xml bridge id names by its last two components."""
        return {
            "plugins.media-keys": self.media_keys,
            "wm.keybindings": self.wm_keybindings,
            "solus-project.ragnar-wm": self.ragnar_wm,
            "mutter.keybindings": self.mutter_keybindings,
            "gnome.mutter": self.mutter,
        }.get(short_schema)
