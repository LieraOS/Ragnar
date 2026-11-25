// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-power-dialog — D-Bus interface definitions for --daemon mode
//
// Mirrors what the legacy legacy ragnar-power-dialog daemon exposed:
//   org.gnome.Shell  → /org/gnome/SessionManager/EndSessionDialog
//     (the interface ragnar-session calls when the user logs out)
//   org.lieraos.ragnar.Session.EndSessionDialog
//     (the ragnar-native twin the legacy shim proxied to — we serve it
//      directly, no indirection)
//   org.lieraos.ragnar.PowerDialog → /org/lieraos/ragnar/PowerDialog
//     (Toggle()/is_showing; kept for compatibility with callers that
//      poke the dialog via this name)

/// Introspection XML for all three interfaces, one <node> covering
/// every object we register.
pub const NODE_XML: &str = r#"<node>
  <interface name="org.lieraos.ragnar.PowerDialog">
    <method name="Toggle"/>
    <signal name="toggle"><arg name="show" type="b"/></signal>
    <property name="is_showing" type="b" access="read"/>
  </interface>
  <interface name="org.lieraos.ragnar.Session.EndSessionDialog">
    <method name="Open">
      <arg name="type" type="u" direction="in"/>
      <arg name="timestamp" type="u" direction="in"/>
      <arg name="open_length" type="u" direction="in"/>
      <arg name="inhibiters" type="ao" direction="in"/>
    </method>
    <method name="Close"/>
    <signal name="ConfirmedLogout"/>
    <signal name="ConfirmedReboot"/>
    <signal name="ConfirmedShutdown"/>
    <signal name="Canceled"/>
    <signal name="Closed"/>
  </interface>
  <interface name="org.gnome.SessionManager.EndSessionDialog">
    <method name="Open">
      <arg name="type" type="u" direction="in"/>
      <arg name="timestamp" type="u" direction="in"/>
      <arg name="open_length" type="u" direction="in"/>
      <arg name="inhibiters" type="ao" direction="in"/>
    </method>
    <method name="Close"/>
    <signal name="ConfirmedLogout"/>
    <signal name="ConfirmedReboot"/>
    <signal name="ConfirmedShutdown"/>
    <signal name="Canceled"/>
    <signal name="Closed"/>
  </interface>
</node>"#;

/// The two object paths the EndSessionDialog interface is served on.
pub const PATH_GNOME: &str = "/org/gnome/SessionManager/EndSessionDialog";
pub const PATH_RAGNAR: &str = "/org/lieraos/ragnar/Session/EndSessionDialog";
/// Object path of the PowerDialog toggle interface.
pub const PATH_POWER: &str = "/org/lieraos/ragnar/PowerDialog";

/// Bus names this daemon owns.
pub const NAME_SHELL: &str = "org.gnome.Shell";
pub const NAME_ENDSESSION: &str = "org.lieraos.ragnar.Session.EndSessionDialog";
pub const NAME_POWER: &str = "org.lieraos.ragnar.PowerDialog";
