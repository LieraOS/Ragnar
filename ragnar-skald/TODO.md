# TODO - LieraAudio Development

## Phase 1: Output Device Control — backend DONE (real PipeWire)

### Core Audio Backend (PipeWire)
- [x] Native PipeWire worker thread (`MainLoop` + `Context` + `Core`)
- [x] Live node enumeration via registry `global`/`global_remove`
      (`Audio/Sink`, `Audio/Source` — real devices, no `pw-cli` parsing)
- [x] Volume control via `ParamType::Props` pods (`channelVolumes` +
      `volume` scalar + `mute`), read back via `subscribe_params`
- [x] Volume boost up to 150 % (`MAX_VOLUME`) — real node gain
- [x] Default endpoint switching via `default` metadata object
      (`default.configured.audio.sink/.source`, wpctl-compatible)
- [x] Core error → `BackendLost` event (daemon disconnect handling)
- [ ] Device profile/configuration switching (EnumParams on `Device`,
      e.g. Stereo/Pro-Audio profiles)
- [ ] Per-channel balance control for surround (channelVolumes offsets)

### GUI Frontend (GTK4)
- [x] Main window with Ausgabe/Eingabe tabs
- [x] Real device list (`ListBox`, default marked with ✓)
- [x] Volume slider 0–100 % + percent readout
- [x] Boost checkbox extending the range to 150 %
- [x] Mute checkbox wired to `Props.mute`
- [x] Live updates on backend events (feedback-guarded)
- [x] Settings persistence into GUI (fx state + sliders)
- [ ] Balance UI for multi-channel endpoints

### Audio effects — native filter-chain DONE
- [x] `libpipewire-module-filter-chain` loaded in-process via
      `pw_context_load_module` (FFI — same mechanism as EasyEffects)
- [x] One module per effect, chained via `playback.props.target.object`
      (loaded in reverse order so link targets exist at link time)
- [x] Built-in effects with PipeWire's own `builtin` plugins:
  - [x] Bass EQ (`bq_lowshelf`, -10..+10 dB)
  - [x] Treble EQ (`bq_highshelf`, -10..+10 dB)
  - [x] Echo (`delay` + dry/wet `mixer` tee topology)
  - [x] Reverb (`convolver` + synthesized impulse response, generated
        at runtime into `$XDG_STATE_HOME/lieraaudio/reverb-ir.wav`)
- [x] Live control via `Props`/`params` pods (`SPA_PROP_params` struct)
      — routed to each module's first graph node
- [x] `effects_widget.rs` wired: master switch + sliders on output tab
- [x] Persisted via `config/persistence.rs` (`audio_config.fx_enabled`,
      debounced writes, chain auto-restores on start)
- [x] Chain teardown restores the previous default sink; module unload
      on backend drop
- [ ] Advanced effects stay optional → EasyEffects for LV2 power users

### Quality / packaging
- [x] `pw-cli` subprocess calls removed — pure `pipewire-rs`
- [x] No `unwrap()`/`expect()` in production paths
- [x] `tokio` removed (glib + pw channel are sufficient)
- [x] Unit + live integration tests green; `clippy`/`fmt` warning-free
- [x] All source files ≤ 400 lines
- [ ] Debian package (`lieraaudio` + `lieraaudio-fx`) + desktop file + icon
- [ ] Package deps for ISO: `swh-plugins`, `rubberband-ladspa`
- [x] Package `librnnoise_ladspa.so` (werman noise-suppression-for-voice)
      as `librnnoise-ladspa_0.2-1_amd64.deb` — ships in
      `Liera/config/packages.chroot`; rebuild via
      `pkg/librnnoise-ladspa/build-deb.sh` (pinned upstream commit);
      also user-installed to `~/.ladspa/` on dev machines
- [ ] Panel integration (speaker icon → LieraAudio)
- [ ] Add to LieraOS LiveBuild `packages.chroot`
- [ ] Multilanguage UI (de/en/es/fr) — GUI strings are still German-only

## Phase 2: Input Device Control — DONE (live verified)

### Microphone effects chain
- [x] `lieraaudio.micfx.*` stages: capture side `target.object` chaining,
      final playback exposes stable virtual source `lieraaudio.mic.out`
      (`Audio/Source`, becomes session default input)
- [x] Voice presets (radio-style `CheckButton` group — `DropDown` is
      broken on this GTK build): Off / Micky Maus / Roboter /
      Frauenstimme / Männerstimme
  - Pitch presets use `swh am_pitchshift` (live "Pitch shift" control
    → Tonhöhe slider -12..+12 st, persisted via `mic_voice_pitch`),
    `rubberband-live-pitchshifter-*`/`swh pitchScale` fallbacks,
    passthrough when no plugin installed; Female (+6 st) and Male (-5 st)
    add static biquad timbre EQ (vocal-tract shaping) in the graph
  - Robot uses `swh ringmod_1i1o1l` (55 Hz LFO, RM depth 2.0)
- [x] Noise suppression via rnnoise (`librnnoise_ladspa`,
      `noise_suppressor_stereo`) with live VAD threshold slider — real
      spectral suppression, not just a gate; falls back to `swh gate`
      (-55..-15 dB) when the plugin is missing
- [x] Light mic echo (built-in `delay`+`mixer`, wet gain 0..0.5)
- [x] LADSPA capability detection (`LADSPA_PATH` + system dirs);
      missing plugins → stage runs as `copy` passthrough, GUI shows
      "(Plugin fehlt)"
- [x] Registry-race fix: `mic_pending` defers enable until both the
      default metadata AND a real source have been enumerated

### Live meter + preview
- [x] `MicMeter` capture stream: F32LE negotiation, peak →
      `MicLevel` events ~20 Hz, all-atomic RT callback (no alloc/block)
- [x] `LevelBar` in the input tab follows the selected source
      (`SetMeterTarget`, no-op on unchanged id)
- [x] "Anhören" preview: `libpipewire-module-loopback` to the current
      output — GUI-owned, dies with the window (feedback safety)

### Persistent daemon (`lieraaudio-fx`)
- [x] Headless binary owns both chains; single-instance via
      Unix-socket lock; GUI spawns it idempotently on enable
- [x] Watches `settings.json` (mtime), applies idempotent commands;
      exits when everything disabled; XDG autostart while enabled
- [x] GUI pushes live slider `params` onto daemon-owned nodes —
      control writes work across processes (registry is global)
- [x] Presence-driven state: `FxStateChanged`/`MicFxStateChanged`
      reflect actual fx nodes in the registry regardless of owner
- [x] Live-verified: real mic → denoise → voice(robot) → echo →
      `lieraaudio.mic.out` (default source `*`), teardown restores
      source 65 and the daemon exits

### Tests
- [x] 17 unit tests (pods, IR, mic args/graph/presets, serde compat)
- [x] 4 live integration tests (enumeration, volume/mute, output chain,
      mic chain incl. plugin graphs via `LADSPA_PATH`)
- [ ] Per-application volume (stream nodes)

## Notes
- `eeffect/` and `pipewire/` directories are reference material only.
- All code comments in English; user-facing text in German.
- GUI avoids `gtk4::DropDown`/`gtk4::Switch` — they stack-overflow in
  layout on some GTK 4.18 builds; `ListBox`/`CheckButton` used instead.
