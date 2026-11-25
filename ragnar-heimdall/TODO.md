# Heimdall (Rust/GTK4) — TODO

## Philosophy

Full independence: no NetworkManager, no GNOME/KDE stack. Everything
uses standard Linux mechanisms — `ip`/sysfs, iwd
(`net.connman.iwd`), systemd-networkd, UFW, wg-quick/OpenVPN —
driven entirely from the GUI with the mouse, no terminal typing.
System-wide changes are confirmed via polkit (`pkexec`).

## Done
- [x] Project skeleton (Cargo, gettext init, adw app, README/TODO)
- [x] Sidebar + stack window matching the classic layout
- [x] Backend: interface snapshot via `ip -json` + sysfs wifi detection
- [x] Backend: default route, DNS resolvers, hostname
- [x] Dashboard and Interfaces views with live data
- [x] Manual refresh button
- [x] WiFi scan via iwd D-Bus (SSID, signal %, band, security —
      lives inside Interfaces as the "Wi-Fi Networks" section;
      `StationDiagnostic` supplies per-BSS freq/BSSID; fixed the
      interface constant — it was wrongly written
      `Station.Diagnostic` so every GetDiagnostics call failed with
      "No matching method" and only the fallback ever ran). Reply
      shapes are decoded wire-accurate: `GetOrderedNetworks` accepts
      both `a(on)` and `a(oi)` signal widths, `GetDiagnostics` both
      `aa{sv}`, the BSSID-keyed `a{sa{sv}}`, and the flat `a{sv}`
      association-stats shape newer iwd emits; a failed diagnostic
      path falls back to ordered networks and reports BOTH errors
- [x] WiFi connect flow: clickable networks, password dialog,
      `Network.Connect` via D-Bus — scan + associate + handshake are
      one atomic iwd operation; passphrase arrives through a
      registered `net.connman.iwd.Agent`. Waits for
      `Station.State=connected` (auth failure/timeout become real
      errors instead of silent "connected"); iwd persists profiles in
      `/var/lib/iwd` itself, persistence verified afterwards
- [x] Live association state: `Station.ConnectedNetwork` is re-read
      every 4 s (read-only property fetch) so autoconnect and drops
      show correctly; explicit "Disconnect" row calls
      `Station.Disconnect` — the link never goes down implicitly
- [x] Interface rows refresh every 3 s from a fresh `ip -json`
      snapshot (read-only): rows update in place so expanded state
      survives, vanished interfaces are removed, new ones appended
- [x] Panel status indicator (`ragnar-heimdall-indicator`, ksni/StatusNotifierItem):
      replaces the nm-applet network icon in the Ragnar panel — live
      icon (wireless + SSID, wired, offline), tooltip with link state,
      left click opens Heimdall, menu offers Disconnect while
      associated. Autostarts via /etc/xdg/autostart `OnlyShowIn=Ragnar`;
      all state reads are passive
- [x] DNS fix for the live image: `/etc/resolv.conf` is created as a
      symlink to resolved's stub via tmpfiles.d `L+` at every boot —
      live-build's resolv restore clobbers any hook-installed link
      with an empty file, which silently killed all name resolution
- [x] DHCP handoff: pkexec'd client only runs when systemd-networkd
      is absent — on the LieraOS stack networkd owns the lease, no
      surprise auth dialog or competing client
- [x] Debian packaging via cargo-deb (`packaging/build-deb.sh`,
      lands in `config/packages.chroot/` of the live build)
- [x] Live-build stack: iwd enabled, systemd-networkd/resolved
      enabled, NetworkManager + wpa_supplicant masked
      (`9850-ragnar-heimdall-network.hook.chroot`), `/etc/iwd/main.conf`
      keeps iwd's own network config off (networkd owns DHCP)
- [x] Firewall page: UFW front-end (on/off switch, default policies,
      add/remove port rules) via `pkexec` + `ragnar-heimdall-helper` and the
      `org.lieraos.ragnar.Heimdall.admin` polkit action (auth_admin_keep)
- [x] Routing page: all routing tables (`ip -j route table all`,
      IPv4 + IPv6 grouped per table, kernel/local entries read-only)
      plus `ip rule` policy list; add/delete routes via a form
      dialog + confirmation through the same pkexec helper
      (`ip route add|del|replace` whitelisted)
- [x] DNS page: systemd-resolved via `resolvectl -j` — global
      DNSSEC/DoT/LLMNR/mDNS switches, per-link resolver list with
      current answering server, preset/custom server dialog
      (Cloudflare/Google/Quad9 or free-form), revert-to-DHCP,
      search domains, cache flush, live resolution test with
      latency; mutations through the same `resolvectl` whitelist
      in the pkexec helper
- [x] Wi-Fi options: MAC randomization mode + range
      (`AddressRandomization`/`AddressRandomizationRange` in
      `/etc/iwd/main.conf`) as combos inside the Interfaces Wi-Fi
      section — helper `iwd set` writes the whitelisted key and
      restarts iwd. No standalone Settings page: too few honest
      options to justify one (AutoConnect is per-profile only,
      PowerSave has no persistent iwd switch)
- [x] VPN page: provider-agnostic profile management — import
      WireGuard `.conf` or OpenVPN `.ovpn/.conf` files into the
      standard system dirs (`/etc/wireguard`,
      `/etc/openvpn/client`), connect/disconnect switches
      (`wg-quick` / `openvpn-client@` units), boot autostart
      toggles, live rx/tx transfer stats while up, profile delete
      with confirmation; all mutations go through the `vpn` verb
      family of the pkexec helper (name whitelist + staged /tmp
      imports only)
- [x] RF-kill preflight: soft/hard-blocked radios are detected before
      connect (scan works on a blocked card, association cannot) and
      reported as an actionable error instead of a timeout
- [x] Connect diagnostics: `Station.State` transition logging,
      timeout error names the last iwd state; secured SSID without
      passphrase/stored profile fails fast; changed passphrase drops
      the stale KnownNetwork via `Forget` first
- [x] Backend rewrite to iwd (root cause of the live-ISO timeouts:
      wpa_supplicant starved SelectNetwork behind a permanently
      pending scan-request on ath9k — invisible at normal log level,
      unreachable from userland; iwd's atomic Connect has no such
      starvation window)
- [x] Live image: `iw`, `rfkill`, `wireless-regdb` shipped;
      `dhcpcd-base` dropped and dhcpcd.service masked (no second DHCP
      client vs networkd); Calamares copies `/var/lib/iwd` profiles
      into the installed system

## Next (in progress)
- [x] Interfaces configurable: link up/down switch + IPv4 dialog
      (DHCP/static via networkd drop-in `60-ragnar-heimdall-<if>.network`),
      per-interface expander rows
- [x] Admin-state fix: enable switch mirrors IFF_UP, carrier shown
      separately ("waiting for cable"); driver/speed/RX-TX info rows;
      DNS + MTU in the IPv4 dialog
- [x] IPv6 toggle per interface (sysctl disable_ipv6, persisted via
      /etc/sysctl.d/) — fixed an inverted enable mapping
- [x] Route priority per interface: automatic/preferred/fallback as
      networkd RouteMetric drop-in (10- prefix so it beats the global
      50- catch-all — the old 60- files never matched)
- [x] Virtual interfaces: create/delete VLAN, bridge, bond with
      member links via whitelisted linkadd/linkmaster/linkdel verbs
- [x] Wi-Fi: connected network pinned top, right-click menu,
      per-SSID properties (AutoConnect, Hidden, AlwaysAskForPassphrase,
      IPv6, passphrase, per-network static IPv4+DNS via iwd-profile)
- [x] Fixed a rename regression: the iwd agent object path contained
      a hyphen (invalid in D-Bus object paths) which broke every
      passphrase connect — now underscores
- [x] Wi-Fi power toggle (rfkill soft-block) + forget button on
      known networks (KnownNetwork.Forget)
- [x] Diagnostics v4: verdict header (pass/warn/fail counts) +
      latency/packet-loss ping, IPv6 default route, WiFi signal dBm
- [x] Diagnostics v5: two-tier — one-click quick battery (6 suites)
      plus 9 switchable expert suites (portal detection, IPv6,
      tracepath/MTU, NTP, neighbors, listeners, VPN, DNS depth,
      Wi-Fi details); markdown report export via FileDialog;
      check names/details run through gettext
- [x] gettext infrastructure + German catalog (`po/de.po` →
      `ragnar-heimdall.mo`, msgfmt step in build-deb.sh)
- [ ] gettext catalogs es/fr (de done; en is the msgid source)
- [ ] Verify iwd connect flow on the real live image
- [x] Panel/tray integration: `ragnar-heimdall-indicator` ships as
      autostart status applet (`OnlyShowIn=Ragnar`)
- [x] Privilege escalation wrapper (`pkexec`) for system-wide changes —
      `ragnar-heimdall-helper` + `org.lieraos.ragnar.Heimdall.admin`

## Views (vision per section)
- **Interfaces** — list + configure each adapter; the Wi-Fi Networks
  section lives here (scan/connect/disconnect/forget on the wireless
  adapter), Ethernet gets DHCP/static IP and link up/down
- **Routing** — done: tables + policy rules shown, routes addable/
  deletable via pkexec
- **DNS** — done: resolved front-end (global switches, per-link
  servers, presets/custom, domains, flush, live test)
- **VPN** — done: WireGuard/OpenVPN profile import + connect +
  autostart + delete; kill-switch later
- **Firewall** — done: UFW front-end with on/off, defaults, port rules
- **Diagnostics** — done (v3): explicit "Run diagnostics" button only,
  NO auto-run; user-selectable suites via switches (links incl. ALL
  per-interface IPs / routing+gateway ping / DNS servers+resolution /
  networkd+iwd+ufw service states / WiFi rfkill+association / internet:
  external IP via cdn-cgi/trace, latency TTFB, download ~25MiB + upload
  8MiB Mbit/s via Cloudflare speedtest + curl; missing curl → Skip).
  Later: traceroute, MTU probing, history/trend storage
- **Settings** — dropped: the only honest global option (MAC
  randomization) lives in the Wi-Fi section of Interfaces instead;
  AutoConnect is per-network, PowerSave cannot persist in iwd

## Later
- [ ] Automatic refresh / live status (netlink event watch instead of
      the 3s poll — rtnetlink pushes link up/down in real time)
- [ ] gettext catalogs es/fr (de done; en is the msgid source)
- [x] .desktop file + icon
- [x] Debian packaging via cargo-deb

## Security hardening
- [x] Privileged helper rewritten from shell to Rust — verb
      whitelist identical, but no shell parsing exists anywhere:
      args are validated and exec'd via argv vectors; validators
      are unit-tested (6 tests)
- [x] Profile save now merges — clone of the loaded profile keeps
      unknown iwd keys; toggled-off options are explicitly removed
- [x] 802.1X enterprise Wi-Fi: .8021x profiles get EAP fields
      (method PEAP/TTLS, outer identity, phase-2 method/identity/
      password, CA cert) in the properties dialog
