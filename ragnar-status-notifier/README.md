# ragnar-status-notifier

Rust-Dienst, der den StatusNotifierWatcher im Ragnar-Desktop übernimmt.
Besitzt `org.freedesktop.StatusNotifierWatcher` und
`org.kde.StatusNotifierWatcher` (jeweils auf dem kanonischen Pfad und
`/StatusNotifierWatcher`) sowie den eigenen Host-Namen des Desktops.
Tray-Items und Hosts registrieren sich per `RegisterStatusNotifierItem` /
`RegisterStatusNotifierHost`; Items werden per Name-Watching verfolgt und
bei Verschwinden wieder entfernt. Spec-Signale laufen auf beiden
Interfaces, dazu die Ragnar-Erweiterungen (`*Ragnar`-Signale,
`GetRegisteredStatusNotifierPathnamesRagnar`) — identischer Vertrag wie
der ersetzte legacy Dienst.

Rust service implementing the StatusNotifierWatcher for the Ragnar
desktop. Owns `org.freedesktop.StatusNotifierWatcher` and
`org.kde.StatusNotifierWatcher` (each on its canonical path and
`/StatusNotifierWatcher`) plus the desktop's own host name. Tray items
and hosts register via `RegisterStatusNotifierItem` /
`RegisterStatusNotifierHost`; items are name-watched and dropped when
they vanish. Spec signals fire on both interfaces, plus the Ragnar
extensions (`*Ragnar` signals, `GetRegisteredStatusNotifierPathnamesRagnar`)
— same contract as the replaced legacy service.
