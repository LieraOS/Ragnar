# LieraAudio - Audio Control Application

## Overview
A native Wayland audio control application written in Rust with GTK4 frontend and PipeWire backend.
Designed to integrate with a custom taskbar - right-click on speaker icon → Properties → opens this application.

## Technology Stack
- **Language**: Rust
- **GUI Framework**: GTK4
- **Display Server**: Wayland (native)
- **Audio System**: PipeWire (native)
- **Reference**: `eeffect/` directory contains Easy Effects source code for reference only (not included in final build)

## Phase 1: Output Device Control

### Output Device Settings
- List all available audio output devices (sound cards, headsets, etc.)
- Device configuration options:
  - Stereo
  - Analog
  - Other audio formats
- Balance settings for surround sound (5.1):
  - Front speakers
  - Rear speakers
  - Subwoofer positioning

### Volume Control
- Standard range: 0-100%
- Boost/Override option:
  - Checkbox to enable volume boost
  - When enabled: range extends to 150%
  - Must be real audio boost (not fake UI)

### Audio Effects (Output)
- **Reverb (Hall)**: 0 to Level 10
- **Echo**: 0 to Level 10
- **Bass/Equalizer**: Adjustable highs and lows

## Phase 2: Input Device Control

- Source enumeration + volume/mute (same code path as output)
- **Microphone effects chain** — virtual `Audio/Source`
  (`lieraaudio.mic.out`) becomes the session default input, so every
  application records the processed signal:
  - Voice presets: Micky Maus (pitch +7 st), Roboter (ring modulator),
    Frauenstimme (+6 st), Männerstimme (-5 st) — gender presets get a
    static timbre EQ (bq shelves) after the shifter so they read as a
    different voice, not falsetto; a live "Tonhöhe" slider (-12..+12 st)
    retunes the shifter per voice while a pitch preset is active
  - Noise suppression via rnnoise (`librnnoise_ladspa`, VAD threshold) —
    falls back to the `swh gate` noise gate when rnnoise is missing
  - Light mic echo (built-in `delay` + `mixer`, capped wet gain)
- **Live level meter** — a capture stream reports the mic peak ~20×/s
  so the user sees the microphone is actually alive
- **"Anhören" preview** — `libpipewire-module-loopback` routes the
  processed mic to the current output (GUI-scoped, dies with the window)

## Persistent effects daemon (`lieraaudio-fx`)

Filter-chain modules live in the PipeWire context of the process that
loaded them — GUI-owned modules would die with the window. For
"enabled stays enabled system-wide until disabled", a small headless
daemon owns the chains:

- Spawned by the GUI on enable (single-instance via a Unix-socket
  lock; idempotent, spawnable any number of times)
- Watches `settings.json` (mtime poll) and applies every change
  idempotently — slider moves never rebuild modules
- Exits by itself once all chains are disabled; XDG autostart entry
  restores the configured state after login
- Slider changes additionally push `params` straight onto the
  (globally visible) fx nodes — PipeWire accepts control writes from
  any client, so live updates work without an IPC protocol

### Optional LADSPA plugins (voice presets / noise gate)

| Plugin | Package | Used for |
|---|---|---|
| `librnnoise_ladspa.so` | built from werman/noise-suppression-for-voice | Noise suppression (preferred) |
| `ladspa-rubberband.so` | `rubberband-ladspa` | Mickey/Female/Male pitch shift |
| `ringmod_1188.so` | `swh-plugins` | Robot voice |
| `gate_1410.so` | `swh-plugins` | Noise reduction (fallback) |
| `pitch_scale_1193.so` | `swh-plugins` | Pitch fallback |

Missing plugins degrade gracefully: the stage runs as a pass-through
and the GUI marks the preset as unavailable (`(Plugin fehlt)`).

## Architecture Notes
- Modular code structure
- All code comments in English
- User communication in German
- PipeWire integration for audio control
- GTK4 for native Wayland GUI

## Development Status
- **Phase 1 backend complete**: real native PipeWire backend
  (registry enumeration, live `Props` volume/mute, 150 % boost,
  default-endpoint switching via `default` metadata)
- GUI wired end-to-end: device list, slider, mute, boost — all real
- **Built-in effects live**: native `filter-chain` modules —
  Bass EQ (`bq_lowshelf`), Treble EQ (`bq_highshelf`), Echo (`delay`),
  Reverb (`convolver` + synthesized IR). No EasyEffects required for
  everyday effects; live control via `SPA_PROP_params`.
- **Phase 2 mic chain live**: denoise (rnnoise/gate) → voice preset → echo
  feeding the virtual `lieraaudio.mic.out` source; verified end-to-end
  against a real microphone (links + default reroute + teardown).
- **Effects persist system-wide** via the `lieraaudio-fx` daemon —
  independent of the GUI window's lifetime.
- `pw-cli`/subprocess code fully removed; `tokio` removed
- Reference material available in `eeffect/` and `pipewire/` directories

## Build & Run
```bash
cargo build --release
./target/release/lieraaudio        # GUI
./target/release/lieraaudio-fx     # fx daemon (usually spawned by the GUI)
```

## Tests
```bash
cargo test        # unit tests + live-daemon integration tests
cargo clippy --all-targets
cargo fmt --check
```
Live tests skip automatically when no PipeWire daemon is reachable.
