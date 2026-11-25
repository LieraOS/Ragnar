# ragnar-greeter — TODO

## Open

- Countdown display for timed autologin ("signing in as X in N s")
- Optional on-screen keyboard button (accessibility)
- Keyboard navigation: Escape should cancel a running auth
  (focus order and Enter-to-next-field are done)
- Verify power buttons on real hardware — in QEMU the daemon
  reported no power capabilities, so nothing was shown

## Done

- Hand-written liblightdm-gobject-1 FFI layer (no dev package
  needed — links the versioned soname)
- Animated LIERA backdrop spanning all monitors: cover-scaled
  artwork, gold beam on random paths, lightning strikes
  (left/right/centre zones, double strikes), twinkling stars;
  panel anchored to the primary monitor
- Login panel under the ornament band (GRUB-style position)
- Editable username entry + user-list menu button, session picker
- Power action row driven by daemon capabilities
- Clock + date overlay
- Keyboard flow: focus starts in the name field, Enter steps
  to the password field
- `--preview` mode + `RAGNAR_GREETER_ARTWORK` env override
- lightdm.conf.d drop-in selecting the greeter session
- Verified against a real daemon via `lightdm --test-mode`
  and end-to-end inside QEMU on the live ISO
