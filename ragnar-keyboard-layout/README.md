# ragnar-keyboard-layout

Rust-Dienst, der die Tastaturbelegungs-Anfragen im Ragnar-Desktop
vermittelt. Besitzt `org.lieraos.ragnar.KeyboardLayout` am Session-Bus:
`SetKeyboardLayout`, `SwitchLayoutNext/Previous` werden als Signale
weitergereicht; die labwc-Bridge wendet das Layout an
(`XKB_DEFAULT_LAYOUT`) und schreibt das Ergebnis in die Read-Write-
Property `CurrentLayout` zurück (inkl. `PropertiesChanged`). Reiner
Broker — identischer Vertrag wie der ersetzte legacy Dienst.

Rust service brokering keyboard-layout requests in the Ragnar desktop.
Owns `org.lieraos.ragnar.KeyboardLayout` on the session bus:
`SetKeyboardLayout`, `SwitchLayoutNext/Previous` are forwarded as
signals; the labwc bridge applies the layout (`XKB_DEFAULT_LAYOUT`)
and writes the result back into the read-write `CurrentLayout`
property (with `PropertiesChanged`). Pure broker — same contract as
the replaced legacy service.
