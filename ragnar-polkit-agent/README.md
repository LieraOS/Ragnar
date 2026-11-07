# ragnar-polkit-agent

**Deutsch**

Polkit-Authentifizierungsagent für den Ragnar-Desktop (Liera) —
ersetzt den alten legacy/GTK3 `ragnar-polkit-dialog`.

Läuft als Session-Dienst (Autostart, Phase `Desktop`) ohne Fenster
und öffnet einen GTK4/libadwaita-Passwortdialog, sobald ein Programm
über polkit Admin-Rechte anfordert (`pkexec`, Ragnar Settings …).

**Architektur (Standard-Linux, kein GNOME):**
- Exportiert `org.freedesktop.PolicyKit1.AuthenticationAgent` auf dem
  **System-Bus** unter `/org/lieraos/ragnar/PolkitAgent` und meldet
  sich über `RegisterAuthenticationAgent` an
  (Subject `unix-session` aus `XDG_SESSION_ID`, Fallback `unix-user`)
- Passwort-Verifikation über `polkit-agent-helper-1` (setuid-Helper,
  PAM-Line-Protokoll) — der Agent berührt PAM nie selbst
- Falsches Passwort → Helper-Respawn im selben `BeginAuthentication`-
  Call (Re-Prompt); Abbrechen → `Cancelled`-Fehler an polkitd

**Build:**

```bash
cargo build --release
packaging/build-deb.sh   # erzeugt target/debian/*.deb
```

---

**English**

Polkit authentication agent for the Ragnar desktop (Liera) —
replaces the legacy legacy/GTK3 `ragnar-polkit-dialog`.

Runs as a session service (autostart, `Desktop` phase) without a
window and opens a GTK4/libadwaita password dialog whenever a
program requests elevated privileges through polkit.

**Architecture (standard Linux, no GNOME):**
- Exports `org.freedesktop.PolicyKit1.AuthenticationAgent` on the
  **system bus** at `/org/lieraos/ragnar/PolkitAgent` and announces
  it via `RegisterAuthenticationAgent` (subject `unix-session` from
  `XDG_SESSION_ID`, fallback `unix-user`)
- Password verification via `polkit-agent-helper-1` (setuid helper,
  PAM line protocol) — the agent never touches PAM itself
- Wrong password → helper respawns inside the same
  `BeginAuthentication` call (re-prompt); Cancel → `Cancelled`
  error back to polkitd

**Build:** see German section above.
