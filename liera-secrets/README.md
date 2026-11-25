# liera-secrets

**Deutsch:** Gemeinsamer Credential-Store für LieraOS-Programme, gebaut auf dem freedesktop Secret Service (`org.freedesktop.secrets` über D-Bus). Die eigentliche Verschlüsselung macht der installierte Backend-Daemon (z. B. gnome-keyring, KeePassXC) — diese Bibliothek spricht nur das Standard-Protokoll, also bleibt jede Komponente austauschbar und niemand baut eigene Krypto.

Nutzung: `ragnar-settings` legt Cloud-Zugänge (Nextcloud/ownCloud/WebDAV) ab, `ragnar-raven` (CalDAV-Sync), `Valkyrie` (WebDAV-Mounts) und weitere lesen sie über dieselbe API. Items liegen in der Default-Collection unter dem Namespace `liera-cloud`, ein Eintrag je `service`-Name.

API: `store(CloudAccount)` / `get(service)` / `delete(service)` / `list()`. Zusätzlich eine kleine CLI `liera-secrets <store|get|delete|list>` zum Testen und für Skripte.

**English:** Shared credential store for LieraOS programs, built on the freedesktop Secret Service (`org.freedesktop.secrets` over D-Bus). Actual encryption is done by the installed backend daemon (e.g. gnome-keyring, KeePassXC) — this library only speaks the standard protocol, so every component stays replaceable and nobody builds their own crypto.

Usage: `ragnar-settings` stores cloud accounts (Nextcloud/ownCloud/WebDAV); `ragnar-raven` (CalDAV sync), `Valkyrie` (WebDAV mounts) and others read them through the same API. Items live in the default collection under the `liera-cloud` namespace, one entry per `service` name.

API: `store(CloudAccount)` / `get(service)` / `delete(service)` / `list()`. Plus a small `liera-secrets <store|get|delete|list>` CLI for testing and scripts.

## Build

```bash
cargo build --release
```
