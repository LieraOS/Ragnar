# ragnar-greeter

Animated LightDM greeter for the Ragnar desktop. Replaces
lightdm-gtk-greeter with a GTK4 fullscreen greeter that carries
the LIERA artwork through the whole boot chain: the same
background as GRUB and Plymouth, the same lightning/glow ambient
animation as the Plymouth splash, and a login panel placed under
the ornament band like the GRUB menu.

## Features

- Fullscreen animated backdrop spanning all monitors: cover-scaled
  LIERA artwork, a gold beam sweeping random paths, lightning
  strikes (left/right/centre, occasional double strikes), and
  twinkling stars in the sky band
- Username: always a free-text entry, plus a user-list button
  when the daemon knows accounts (honours `hide-users`)
- Password entry with peek toggle; Enter in the name field steps
  to the password
- Session dropdown (auto-hidden when only one session exists)
- Power actions (suspend / hibernate / restart / shutdown), only
  for actions the daemon advertises
- Clock + date overlay, dim-gold styled
- Timed autologin honouring the daemon's hint
- German UI strings with an English gettext catalog (`po/en.po`)

## How it works

The greeter runs as LightDM's `greeter-session` inside its own X
server — there is no window manager, so the window is sized to
the bounding box of all monitors directly; the login panel stays
centred on the primary monitor via the scene's anchor rect.
Authentication talks to
the lightdm daemon through `liblightdm-gobject-1` via a small
hand-written FFI layer (`src/ffi.rs`); the crate links the
versioned soname, so `liblightdm-gobject-dev` is not needed to
build.

## Build

```sh
cargo build --release
cargo test
```

Runtime dependencies: `liblightdm-gobject-1-0`, GTK4.

## Preview without LightDM

```sh
cargo run -- --preview        # windowed, no daemon needed
RAGNAR_GREETER_ARTWORK=/path  # override artwork directory
```

## Install / packaging

`cargo deb` produces the `.deb` with:

- `/usr/bin/ragnar-greeter`
- `/usr/share/ragnar-greeter/artwork/*`
- `/usr/share/xgreeters/ragnar-greeter.desktop`
- `/etc/lightdm/lightdm.conf.d/50-ragnar-greeter.conf`
  (`greeter-session=ragnar-greeter`)
- English locale catalog

On the live ISO nothing changes — autologin skips the greeter.
On installed systems the new greeter appears at the next boot.
