# LieraAudio Architecture Design

## Overview

Modular audio control application with a **real native PipeWire backend**
(no `pw-cli`/`pactl` subprocesses) and a GTK4 frontend. Designed for
LieraOS/Wayland; integrates as the properties dialog of the panel
speaker icon.

## Threading model

```
GTK main thread                          pw-worker thread
----------------                         ---------------------------
Application / Window                     pipewire::MainLoop
EndpointPanel (GUI)  -- BackendCommand -> channel::Receiver -> Tracker
                   <- BackendEvent ----  (std::sync::mpsc)
                   (drained by a 50 ms   Registry listener binds
                    glib timer)          node proxies, subscribes
                                         Props params, binds the
                                         "default" metadata object
```

- All PipeWire objects live on a dedicated worker thread running
  `pw_main_loop`.
- Commands GUI → worker use `pipewire::channel` (wakes the pw loop).
- Events worker → GUI use `std::sync::mpsc`, drained every 50 ms by a
  `glib::timeout_add_local` callback on the main context.
- Widget signal handlers never touch PipeWire; they only emit
  `BackendCommand`s. A `updating` guard suppresses command echo when
  widgets are synced from backend events.

## Project structure

```
lieraaudio/
├── Cargo.toml                 # Dependencies (pipewire + libspa 0.8, gtk4 0.9)
├── src/
│   ├── main.rs               # Entry point (logging + application start)
│   ├── lib.rs                # Library exports
│   ├── pipewire/             # Real PipeWire backend
│   │   ├── mod.rs            # Module exports
│   │   ├── backend.rs        # PwBackend — GUI-facing handle
│   │   ├── worker.rs         # Worker thread, main loop, listeners
│   │   ├── tracker.rs        # Node proxies, endpoint state, registry
│   │   ├── tracker_meta.rs   # Session defaults via "default" metadata
│   │   ├── tracker_fx.rs     # Fx module lifecycle + runtime control
│   │   ├── fxchain.rs        # Filter-chain graph args + module FFI
│   │   ├── ir.rs             # Synthesized reverb impulse response
│   │   ├── props.rs          # SPA Props pod serialize/parse (unit-tested)
│   │   └── types.rs          # AudioEndpoint, BackendCommand, BackendEvent
│   ├── audio/                # Effect/balance models (Phase: filter-chain)
│   │   ├── effects.rs        # Reverb/Echo/EQ settings model
│   │   ├── balance.rs        # Channel balance model
│   │   └── config.rs         # AudioConfig aggregate
│   ├── gui/
│   │   ├── mod.rs            # GUI module exports
│   │   ├── application.rs    # GTK4 application + backend startup
│   │   ├── window.rs         # Main window, tab stack, event drain
│   │   ├── endpoint_panel.rs # Reusable per-direction control panel
│   │   ├── output_panel.rs   # Output tab (Direction::Output)
│   │   ├── input_panel.rs    # Input tab (Direction::Input)
│   │   └── effects_widget.rs # Built-in effects UI (wired to backend)
│   └── config/
│       ├── settings.rs       # AppSettings model
│       └── persistence.rs    # JSON config in ~/.config/lieraaudio
└── tests/
    └── pipewire_live.rs      # Live-daemon integration tests (self-skip)
```

## Backend details

- **Enumeration**: `pw_registry` `global`/`global_remove` events; nodes
  filtered by `media.class` (`Audio/Sink` → output, `Audio/Source` →
  input). Display name from `node.description`/`node.nick`/`node.name`.
- **Volume/mute**: `pw_node_set_param(ParamType::Props)` with an SPA pod
  carrying `channelVolumes` (flat on all channels) + `volume` scalar +
  `mute`. Current state comes from `subscribe_params` param events.
- **Boost**: linear gain up to `MAX_VOLUME = 2.0` (200 %) — real node
  gain, identical mechanism to `wpctl set-volume 1.5`.
- **Default endpoint**: writing `default.configured.audio.sink|.source`
  on the bound `default` metadata object — the same mechanism
  `wpctl set-default` uses (WirePlumber applies it session-wide).

## Built-in effects (filter-chain)

Each effect is a separate `libpipewire-module-filter-chain` instance
loaded into the client context via `pw_context_load_module` — the same
mechanism EasyEffects uses, but no LV2 plugins needed (PipeWire `builtin`
plugins only). One module per effect is required because PipeWire routes
`params` control updates to the first graph node only.

```
apps → fx.bass.in → treble.in → echo.in → reverb.in → default sink
```

- Modules load in **reverse** order so each `playback.props.target.object`
  already exists when WirePlumber links the playback stream; the last
  module has no target → lands on the session default sink (follows
  output-device changes).
- `capture.props.media.class = Audio/Sink` on the chain entry presents a
  normal sink ("LieraAudio Effekte") that apps can also pick directly.
- Controls: `Props` pod with `SPA_PROP_params` = struct of name/value
  pairs (biquad `Gain` in dB, delay `Feedforward`, mixer `Gain 2`).
- Reverb IR is synthesized at runtime (decorrelated decaying noise,
  48 kHz stereo wav) — no shipped binary blob.
- Teardown restores the previous default sink and destroys modules.

### PipeWire version compatibility

- `delay` requires `config = { max-delay = N }` on PipeWire ≤ 1.4 —
  without it instantiation fails with EINVAL ("can't start graph") and
  the chain goes silent. `Feedback`/`Feedforward` control ports only
  exist in newer versions, so echo level is mixed via `mixer Gain 2`
  instead.

## Testing

- `src/pipewire/props.rs` unit tests: SPA pod round-trips (serialization
  and parsing verified byte-exact locally, no daemon needed).
- `src/pipewire/ir.rs` unit tests: IR decay/bounds, L/R decorrelation.
- `tests/pipewire_live.rs`: live-daemon integration tests that
  self-skip when no PipeWire socket is available — enumeration,
  volume set + daemon-confirmed change, mute round-trip, fx chain
  load → endpoint appears → control write → unload.

## Notes / quirks

- `gtk4::DropDown` and `gtk4::Switch` stack-overflow in layout on some
  GTK 4.18 builds (observed on the dev machine). LieraAudio uses a
  `ListBox` for device selection and a `CheckButton` for mute — same
  UX as GNOME sound settings.
- `wpctl get-volume` reports WirePlumber's *stored* volume; the node's
  live `Props` (what LieraAudio shows/edits) may differ until WP
  reconciles. Node `Props` are the actual applied gain.
- `eeffect/` (EasyEffects) and `pipewire/` source trees are reference
  material only — not compiled, not linked.
