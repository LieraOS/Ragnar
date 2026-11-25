# Ragnar Heimdall

**Deutsch** | [English](#english)

Natives Netzwerk-Kontrollzentrum für den Ragnar-Desktop — Rust,
GTK4/libadwaita, komplett unabhängig von NetworkManager und
GNOME/KDE-Stacks. Früher „lieranet"; der neue Name vermeidet die
Kollision mit dem Heimdall-Flash-Tool.

## Was Heimdall kann

- **Dashboard** — Live-Überblick über Links, Verbindung, Standardroute
- **Schnittstellen** — alle Kernel-Interfaces als aufklappbare Zeilen
  mit Live-Refresh: MAC, MTU, Adressen (IPv4+IPv6), Treiber,
  Link-Geschwindigkeit und Empfangen/Gesendet-Zähler
  - **Aktivierungs-Schalter zeigt den Admin-State** (IFF_UP), nicht
    den Carrier — ein abgezogenes Kabel zeigt „Aktiviert — wartet auf
    Kabel" statt falsch abgeschaltet
  - **IPv4-Konfiguration** pro Interface: DHCP/statisch mit Adresse,
    Gateway, eigenen DNS-Servern (resolvectl) und MTU — persistent
    als networkd-Datei `10-ragnar-heimdall-<if>.network`
  - **IPv6 ein/aus** pro Interface (sysctl `disable_ipv6`, persistent
    über `/etc/sysctl.d/`)
  - **Routen-Priorität** pro Interface: Automatisch / Bevorzugt /
    Ausweichverbindung — als RouteMetric-Drop-in; damit bestimmt der
    Benutzer, ob z. B. LAN vor WLAN geht
  - **Verbindungs-Priorität (global)** oben auf der Seite:
    Automatisch / LAN bevorzugen / WLAN bevorzugen — setzt die
    Metriken aller Links konsistent in einem Schritt
  - **Virtuelle Interfaces** (Erweitert-Gruppe): VLAN, Bridge und
    Bond anlegen — mit Mitglieds-Schnittstellen — und wieder löschen
- **Wi-Fi** — scannen + verbinden über iwds D-Bus-API
  (`net.connman.iwd`): ein atomarer `Network.Connect`-Aufruf
  übernimmt Scan + Assoziieren + Handshake; Passwörter liefert ein
  registrierter `net.connman.iwd.Agent`. Das verbundene Netz steht
  immer oben; Rechtsklick öffnet Verbinden/Trennen, Eigenschaften
  und Vergessen.
- **Netzwerk-Eigenschaften (pro SSID)** — volle iwd-Profil-Kontrolle:
  AutoConnect, verborgenes Netz (Hidden), „Immer nach Passwort
  fragen" (AlwaysAskForPassphrase), IPv6 ein/aus, Passphrase ändern
  und **statische IPv4 pro Netzwerk**. Enterprise-802.1X-Profile (.8021x) werden mit EAP-Method, Identitäten, Phase-2 und CA-Zertifikat editiert. Speichern merged — unbekannte Keys bleiben erhalten (Adresse/Netzmaske/Gateway/DNS)
  — feste IP nur in diesem Netz, überall sonst DHCP. Geschrieben
  wird über den Helper-Verb `iwd-profile` direkt in
  `/var/lib/iwd/<ssid>.psk`
- **iwd-Optionen** — Radio-Einstellungen direkt aus `main.conf`
- **Firewall (UFW)** — Ein/Aus, Standard-Policies für ein-/ausgehend,
  einfache Port-Regeln. **Erweitert-Sektion:** nummerierte Regeln,
  Umsortieren, Bearbeiten und die volle UFW-Regelsyntax (Aktion
  allow/deny/reject/limit, Interface, Log-Flag, Kommentar,
  Einfügeposition) in einem eigenen Fenster
- **Routing** — Routing-Tabellen und Policy-Regeln, Add/Delete
- **DNS** — Resolver-Verwaltung
- **VPN** — WireGuard/OpenVPN-Profile
- **Diagnose, zweistufig** — *Schnelldiagnose* per Ein-Klick
  (Links, Adressen, Gateway, DNS, Dienste, WLAN, Internet-
  Geschwindigkeit) plus **Experten-Prüfungen** als schaltbare
  Batterie: Captive-Portal-Erkennung, IPv6-Erreichbarkeit,
  Tracepath + Path-MTU, NTP-Sync, ARP/NDP-Nachbartabelle,
  lauschende Sockets, VPN-Tunnel-Zustand, DNS-Latenz/-Posture
  und WLAN-Linkdetails via `iw`. Jede Diagnose lässt sich als
  Markdown-Bericht speichern

## Architektur

- **Kein NetworkManager** — nur Standard-Linux-Mechanismen:
  `ip -json`/sysfs für Zustand, `net.connman.iwd` für Wi-Fi,
  systemd-networkd-nahe Werkzeuge für Links, UFW fürs Firewalling,
  wg-quick/OpenVPN für VPN
- **Privilegiert über polkit** — Mutationen laufen über `pkexec` +
  `org.lieraos.ragnar.Heimdall.admin` (`auth_admin_keep`); der
  whitelistende Helper ist eine Rust-Binary
  (`usr/lib/ragnar-heimdall/ragnar-heimdall-helper`) — kein
  Shell-Parsing, argv-Vektoren, validierte Argumente — und erlaubt
  nur exakt die benötigten Verb-Ketten für `ufw`, `ip link/route`,
  `resolvectl`, `sysctl` (IPv6), networkd-Dateien, `iwd-profile`
  und iwd-Optionen — Interface-Namen, VLAN-IDs, Bond-Modi, CIDR
  und Metriken werden auf beiden Seiten validiert
- **Fehlertolerant** — jeder Lesefehler degradiert zu einer sichtbaren
  Statuszeile statt zum Absturz

## Paket

`ragnar-heimdall` — Binaries `ragnar-heimdall` (UI),
`ragnar-heimdall-indicator` (Tray/Autostart, `OnlyShowIn=Ragnar`)
und `ragnar-heimdall-helper` (privilegierter Rust-Helper unter
`/usr/lib/ragnar-heimdall/`). Gettext-Domain `ragnar-heimdall`.

## Abhängigkeiten

**Laufzeit (harte Depends):** `iwd`, `systemd` (networkd +
resolved), `ufw`, `pkexec`/`polkitd` — sowie GTK4/libadwaita aus
`$auto`.

**Empfohlen (Recommends, Checks zeigen sonst „skipped"):**
`iputils-ping`, `iputils-tracepath`, `curl`, `iw`, `rfkill`,
`wireguard-tools`, `openvpn`.

**Build:** Rust ≥ 1.85, GTK4-Dev, libadwaita-Dev, gettext
(`msgfmt`), cargo-deb für das Paket.

## Lizenz

GPL-3.0-or-later — wie alle Ragnar-Komponenten.

## Layout

```
src/
├── app.rs               → AdwApplication bootstrap
├── i18n.rs              → gettext
├── backend/
│   ├── model.rs         → NetworkInterface, LinkKind, LinkState
│   ├── collector.rs     → Interface-Snapshot via ip -json + sysfs
│   ├── system.rs        → Standardroute, DNS, Hostname, DHCP
│   ├── ufw.rs           → UFW-Status/Regeln via pkexec-Helper
│   ├── rfkill.rs        → Funk-Schalter via sysfs
│   ├── routing.rs       → Routing-Tabellen/Policy-Regeln
│   ├── dns.rs           → Resolver
│   ├── vpn.rs           → VPN-Profile
│   ├── iwdconf.rs       → iwd main.conf Optionen
│   └── wifi/            → Scan, Connect, Agent, KnownNetwork
├── ui/
│   ├── window.rs        → Fenster/Sidebar
│   ├── dashboard.rs
│   ├── interfaces.rs    → Schnittstellen + Wi-Fi-Sektion
│   ├── ipv4_add.rs      → IPv4-Dialog (DHCP/statisch/DNS/MTU)
│   ├── iwd_opts.rs      → iwd-Optionen (MAC-Randomisierung)
│   ├── vlink_add.rs     → VLAN/Bridge/Bond-Erstellen + Löschen
│   ├── firewall.rs      → einfache Firewall-Seite
│   ├── firewall_add.rs  → Port-Dialog
│   ├── firewall_adv.rs  → Erweitert-Sektion + Regel-Editor
│   ├── routing.rs / routing_add.rs
│   ├── dns.rs / dns_add.rs
│   ├── vpn.rs / vpn_add.rs
│   ├── diagnostics.rs
│   └── wifi/            → Wi-Fi-Listen und Verbindungs-Flow
└── main.rs
```

---

<a id="english"></a>

## English

Native network control center for the Ragnar desktop — Rust,
GTK4/libadwaita, fully independent of NetworkManager and GNOME/KDE
stacks. Formerly "lieranet"; renamed to avoid colliding with the
Heimdall flashing tool.

### Features

- **Dashboard** — live overview of links, connectivity, default route
- **Interfaces** — every kernel interface as an expandable row with
  live refresh: MAC, MTU, addresses (IPv4+IPv6), driver, link speed
  and received/sent counters
  - **Enable switch mirrors the administrative state** (IFF_UP), not
    the carrier — an unplugged cable shows "Enabled — waiting for
    cable" instead of a false off state
  - **IPv4 configuration** per interface: DHCP/static with address,
    gateway, custom DNS servers (resolvectl) and MTU — persisted as
    networkd file `10-ragnar-heimdall-<if>.network`
  - **IPv6 on/off** per interface (`disable_ipv6` sysctl, persisted
    via `/etc/sysctl.d/`)
  - **Route priority** per interface: automatic / preferred /
    fallback — written as a RouteMetric drop-in so the user decides
    whether e.g. LAN beats Wi-Fi
  - **Connection preference (global)** on top of the page: automatic
    / prefer LAN / prefer Wi-Fi — applies consistent metrics across
    all links in one step
  - **Virtual interfaces** (Advanced group): create and remove
    VLANs, bridges and bonds — including member interfaces
- **Wi-Fi** — scan + connect via iwd's D-Bus API: one atomic
  `Network.Connect` call handles scan + association + handshake;
  passphrases arrive through a registered `net.connman.iwd.Agent`.
  The connected network always pins to the top; right-click offers
  connect/disconnect, properties and forget.
- **Per-network properties** — full iwd profile control per SSID:
  AutoConnect, hidden network, "always ask for passphrase", IPv6
  on/off, passphrase change and **per-network static IPv4**
  (address/netmask/gateway/DNS) — a fixed address only on this
  network, DHCP everywhere else. Written via the `iwd-profile`
  helper verb into `/var/lib/iwd/<ssid>.psk`
- **iwd options** — radio settings straight from `main.conf`
- **Firewall (UFW)** — on/off, default policies, simple port rules.
  **Advanced section:** numbered rules, reordering, editing and the
  full UFW rule syntax (allow/deny/reject/limit, interface, log flag,
  comment, insert position) in a dedicated window
- **Routing**, **DNS**, **VPN**
- **Diagnostics, two-tiered** — a one-click *quick battery*
  (links, addresses, gateway, DNS, services, Wi-Fi, internet
  speed) plus switchable **expert checks**: captive-portal
  detection, IPv6 reachability, tracepath + path-MTU, NTP sync,
  ARP/NDP neighbor table, listening sockets, VPN tunnel state,
  DNS latency/posture and Wi-Fi link details via `iw`. Every
  run exports as a markdown report

### Architecture

Standard Linux mechanisms only — `ip -json`/sysfs for state,
`net.connman.iwd` for Wi-Fi, UFW for firewalling, wg-quick/OpenVPN
for VPN. Privileged changes go through `pkexec` + a whitelisting
helper binary (Rust — no shell anywhere; `ufw`, `ip link/route`,
`resolvectl`, `sysctl`, networkd files, `iwd-profile`) — interface
names, VLAN IDs, bond modes, CIDR and metrics are validated on
both sides; every read failure degrades to a visible status row.

### Dependencies

**Runtime (hard depends):** `iwd`, `systemd` (networkd + resolved),
`ufw`, `pkexec`/`polkitd` — plus GTK4/libadwaita via `$auto`.

**Recommended (checks report "skipped" when absent):**
`iputils-ping`, `iputils-tracepath`, `curl`, `iw`, `rfkill`,
`wireguard-tools`, `openvpn`.

**Build:** Rust ≥ 1.85, GTK4 dev, libadwaita dev, gettext (`msgfmt`),
cargo-deb for packaging.

### License

GPL-3.0-or-later — like every Ragnar component.
