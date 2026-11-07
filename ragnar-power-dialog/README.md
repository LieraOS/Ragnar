# ragnar-power-dialog

**Deutsch**

Sitzungs-Auswahldialog für den Ragnar-Desktop (Liera). Wird auf
Abruf gestartet — aus dem Power-Knopf im Startmenü der Taskbar,
einer Tastenkombination oder einem Menüeintrag — und zeigt große
Aktions-Kacheln: Bildschirm sperren, Abmelden, Bereitschaft,
Ruhezustand, Neustart, Herunterfahren.

**Backends (echt, kein GNOME):**
- Sperren → `org.lieraos.ragnar.Screenlock` (D-Bus, darunter gtklock)
- Abmelden/Neustart/Herunterfahren → `ragnar-session-quit`
- Bereitschaft/Ruhezustand → `systemctl suspend|hibernate` (logind)

**Konfiguration:** GSettings-Schema `org.lieraos.ragnar.session`
(mitgeliefert). Pro Aktion ein `show-*`-Schalter — geschrieben von
Ragnar Settings → Energie, gelesen vom Dialog. Was die Hardware
nicht kann (logind `Can*`), wird zusätzlich automatisch ausgeblendet.

**Theming:** Der Dialog lädt `~/.config/ragnar-desktop/power-dialog.css`
mit USER-Priorität; die mitgelieferte
`/usr/share/ragnar-power-dialog/power-dialog.css` dient als Vorlage.
CSS-Klassen: `.power-dialog`, `.power-action`, `.power-action-icon`,
`.power-action-label`, `.power-action-destructive`,
`.power-dialog-cancel`.

**Build:**

```bash
cargo build --release
packaging/build-deb.sh   # erzeugt target/debian/*.deb
```

---

**English**

Session action chooser for the Ragnar desktop (Liera). Launched on
demand — from the taskbar start-menu power button, a keybind or a
menu entry — and shows large action tiles: lock screen, log out,
suspend, hibernate, restart, shut down.

**Backends (real, no GNOME):**
- Lock → `org.lieraos.ragnar.Screenlock` (D-Bus, gtklock underneath)
- Log out / restart / shut down → `ragnar-session-quit`
- Suspend / hibernate → `systemctl suspend|hibernate` (logind)

**Configuration:** GSettings schema `org.lieraos.ragnar.session`
(shipped). One `show-*` switch per action — written by Ragnar
Settings → Power, read by the dialog. Actions the hardware cannot
perform (logind `Can*`) are hidden automatically as well.

**Theming:** the dialog loads
`~/.config/ragnar-desktop/power-dialog.css` at USER priority; the
shipped `/usr/share/ragnar-power-dialog/power-dialog.css` is the
template to copy. CSS classes: `.power-dialog`, `.power-action`,
`.power-action-icon`, `.power-action-label`,
`.power-action-destructive`, `.power-dialog-cancel`.

**Build:** see German section above.
