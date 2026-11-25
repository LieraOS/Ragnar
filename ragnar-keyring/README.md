# ragnar-keyring

## Deutsch

Startet `gnome-keyring-daemon` zuverlässig unter Ragnar und setzt die
Entsperr-Richtlinie für den Login-Schlüsselbund um.

Hintergrund: Die Stock-Autostart-Dateien von gnome-keyring tragen
`OnlyShowIn=GNOME;Unity;MATE` — unter Ragnar läuft der Daemon daher nie.
Bei Autologin sieht PAM (`pam_gnome_keyring`) kein Passwort, der Bund
bleibt gesperrt, und Apps (Discord, Devin, …) lösen beim ersten
Secret-Zugriff einen Entsperr-Dialog aus.

Dieser Dienst:
1. startet `gnome-keyring-daemon --components=secrets,pkcs11`,
2. füttert — wenn `auto-unlock` aktiv ist (Standard) — ein leeres
   Master-Passwort an `gnome-keyring-daemon --unlock`. Existiert noch
   kein `login.keyring`, wird ein unverschlüsselter Bund angelegt —
   exakt das Verhalten, das Nutzer von anderen Desktops kennen (und
   von Chromiums Plaintext-Fallback). Hat der Bund bereits ein echtes
   Passwort, schlägt der Versuch still fehl und der normale Dialog
   erscheint — nichts bricht.

Richtlinie: `~/.config/ragnar-keyring/settings.ini`
(`[keyring] auto-unlock=true|false`), von ragnar-settings
(Privacy-Panel) editierbar.

## English

Reliably starts `gnome-keyring-daemon` under Ragnar and applies the
login-keyring unlock policy.

Background: gnome-keyring's stock autostart files carry
`OnlyShowIn=GNOME;Unity;MATE`, so the daemon never starts under
Ragnar. With autologin, PAM (`pam_gnome_keyring`) never sees a
password, the keyring stays locked, and apps (Discord, Devin, …)
trigger an unlock dialog on first secret access.

This helper:
1. starts `gnome-keyring-daemon --components=secrets,pkcs11`,
2. when `auto-unlock` is enabled (default), feeds an empty master
   password to `gnome-keyring-daemon --unlock`. If `login.keyring`
   does not exist yet, an unencrypted keyring is created — matching
   the silent behaviour known from other desktops (and Chromium's
   plaintext fallback). If the keyring already has a real password,
   the attempt fails silently and the normal prompt appears — nothing
   breaks.

Policy: `~/.config/ragnar-keyring/settings.ini`
(`[keyring] auto-unlock=true|false`), editable via ragnar-settings
(Privacy panel).
