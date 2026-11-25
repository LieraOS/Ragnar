// Copyright (C) Jörg Schröder
//! Shared types between the PipeWire worker thread and the GTK frontend.
//!
//! The backend owns no GTK types and the frontend never touches PipeWire
//! objects directly; this module defines the whole contract between them.

/// Direction of an audio endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Playback device ("Audio/Sink") — speakers, headphones, HDMI out.
    Output,
    /// Capture device ("Audio/Source") — microphones, line-in.
    Input,
}

impl Direction {
    /// PipeWire `media.class` string for this direction.
    pub fn media_class(self) -> &'static str {
        match self {
            Direction::Output => "Audio/Sink",
            Direction::Input => "Audio/Source",
        }
    }

    /// Metadata key that stores the configured default endpoint name.
    ///
    /// WirePlumber/pipewire-pulse evaluate these keys on the "default"
    /// metadata object; writing them is how `wpctl set-default` works.
    pub fn default_metadata_key(self) -> &'static str {
        match self {
            Direction::Output => "default.configured.audio.sink",
            Direction::Input => "default.configured.audio.source",
        }
    }
}

/// A live application audio stream (`Stream/Output/Audio` node) —
/// one per app currently playing into a sink. Appears when the app
/// starts playing, disappears when it stops: the output page maps
/// each stream to a dynamic volume card.
#[derive(Debug, Clone)]
pub struct AppStream {
    /// PipeWire object id (registry serial).
    pub id: u32,
    /// Stable node name, e.g. `Firefox` or `libcanberra`.
    pub name: String,
    /// Application display name (`application.name` /
    /// `media.name` fallback to node name).
    pub app_name: String,
    /// Stream volume in cubic scale — the value wpctl, pavucontrol and the taskbar show (1.0 = 100 %).
    pub volume: f32,
    /// Mute state.
    pub muted: bool,
}

/// Snapshot of a single audio endpoint (sink or source node).
///
/// `volume` is cubic scale: 1.0 = 100 %. Values above 1.0 are only
/// reachable when the user enables boost in the UI.
#[derive(Debug, Clone)]
pub struct AudioEndpoint {
    /// PipeWire object id (registry serial).
    pub id: u32,
    /// Stable node name, e.g. `alsa_output.pci-...analog-stereo`.
    pub name: String,
    /// Human readable name for the UI (`node.description`).
    pub description: String,
    /// Direction of this endpoint.
    pub direction: Direction,
    /// Master volume in cubic scale (1.0 = 100 %) — same convention as wpctl/pavucontrol.
    pub volume: f32,
    /// Per-channel volumes; length == channel count once known.
    pub channel_volumes: Vec<f32>,
    /// Mute state.
    pub muted: bool,
    /// Whether this endpoint is the session default.
    pub is_default: bool,
}

impl AudioEndpoint {
    /// True while volume information has not yet been received from
    /// PipeWire (param event still pending after node bind).
    pub fn volume_known(&self) -> bool {
        !self.channel_volumes.is_empty()
    }
}

/// Built-in effect identifiers (one filter-chain module each).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FxEffect {
    /// `bq_lowshelf` — bass shelf gain in dB (-10..+10).
    Bass,
    /// `bq_highshelf` — treble shelf gain in dB (-10..+10).
    Treble,
    /// `delay` + dry/wet `mixer` — echo level 0..10 = wet gain.
    Echo,
    /// `convolver` + dry-mix — reverb level 0..10 mapped to wet gain.
    Reverb,
}

impl FxEffect {
    /// Stable key used in node names (`skald.fx.<key>.in`).
    pub fn key(self) -> &'static str {
        match self {
            FxEffect::Bass => "bass",
            FxEffect::Treble => "treble",
            FxEffect::Echo => "echo",
            FxEffect::Reverb => "reverb",
        }
    }

    /// Look an effect up by its node-name key.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "bass" => Some(Self::Bass),
            "treble" => Some(Self::Treble),
            "echo" => Some(Self::Echo),
            "reverb" => Some(Self::Reverb),
            _ => None,
        }
    }

    /// Human-readable label for module descriptions.
    pub fn label(self) -> &'static str {
        match self {
            FxEffect::Bass => "Bass",
            FxEffect::Treble => "Treble",
            FxEffect::Echo => "Echo",
            FxEffect::Reverb => "Reverb",
        }
    }

    /// `params` control name this effect's slider drives plus the
    /// slider→value scaling (slider is 0..10 or -10..+10 in the UI).
    /// Returns `(control_name, scale)`; applied value = `scale * value`.
    pub fn control(self) -> (&'static str, f32) {
        match self {
            // biquad shelf gain is natively in dB — 1:1 mapping.
            FxEffect::Bass | FxEffect::Treble => ("Gain", 1.0),
            // Echo intensity = wet gain of the dry/wet mixer, 0..1
            // (PipeWire ≤ 1.4 `delay` has no Feedforward control, so
            // the mixer's "Gain 2" carries the level).
            FxEffect::Echo => ("Gain 2", 0.1),
            // Reverb level = wet gain of the dry/wet mixer, 0..1.
            FxEffect::Reverb => ("Gain 2", 0.1),
        }
    }
}

/// Built-in input (microphone) effect identifiers — one filter-chain
/// module each, chained on the capture side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MicEffect {
    /// Real noise suppression (rnnoise LADSPA when installed, swh
    /// `gate` as fallback) — cleanup stage before the fun effects.
    Denoise,
    /// Voice transformation (pitch shift / ring modulator). The
    /// concrete graph depends on the active [`VoicePreset`].
    Voice,
    /// `delay` + dry/wet `mixer` — light mic echo, same graph as the
    /// output chain's [`FxEffect::Echo`].
    Echo,
    /// Transparent passthrough — keeps the virtual mic alive while no
    /// effect stage is active. `skald.mic.out` must exist
    /// permanently so every app lands on a stable software-gain
    /// source instead of driving the hardware ADC directly.
    Pass,
}

impl MicEffect {
    /// Stable key used in node names (`skald.micfx.<key>.in`).
    pub fn key(self) -> &'static str {
        match self {
            MicEffect::Denoise => "denoise",
            MicEffect::Voice => "voice",
            MicEffect::Echo => "echo",
            MicEffect::Pass => "pass",
        }
    }

    /// Look an effect up by its node-name key.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "denoise" => Some(Self::Denoise),
            "voice" => Some(Self::Voice),
            "echo" => Some(Self::Echo),
            "pass" => Some(Self::Pass),
            _ => None,
        }
    }

    /// Human-readable label for module descriptions.
    pub fn label(self) -> &'static str {
        match self {
            MicEffect::Denoise => "Noise Suppression",
            MicEffect::Voice => "Voice",
            MicEffect::Echo => "Mic Echo",
            MicEffect::Pass => "Microphone",
        }
    }

    /// `params` control name for the live slider plus
    /// `(scale, offset)`: applied value = `offset + value * scale`.
    /// Returns `None` for stages whose runtime mapping depends on the
    /// installed plugin (denoise) or that rebuild instead of pushing
    /// params (voice) — [`micfx::effect_control`] resolves those.
    pub fn control(self) -> Option<(&'static str, f32, f32)> {
        match self {
            MicEffect::Denoise | MicEffect::Voice | MicEffect::Pass => None,
            // Mic echo wet gain, slider 0..10 → 0..0.5 (deliberately
            // capped lower than the output echo — mic monitoring must
            // stay subtle).
            MicEffect::Echo => Some(("Gain 2", 0.05, 0.0)),
        }
    }
}

/// Selectable voice transformation preset. `Off` means the voice stage
/// is skipped entirely and the mic chain passes the signal through.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum VoicePreset {
    /// No voice transformation.
    #[default]
    Off,
    /// High-pitched cartoon voice (pitch up ~7 semitones, no formant
    /// correction — deliberate chipmunk effect).
    Mickey,
    /// Ring-modulated metallic voice (swh `ringmod`).
    Robot,
    /// Pitch up ~4 semitones with formant preservation.
    Female,
    /// Pitch down ~4 semitones with formant preservation.
    Male,
}

impl VoicePreset {
    /// All presets in UI order.
    pub const ALL: &'static [VoicePreset] = &[
        VoicePreset::Off,
        VoicePreset::Mickey,
        VoicePreset::Robot,
        VoicePreset::Female,
        VoicePreset::Male,
    ];

    /// Base semitone shift of a pitch preset — `None` for presets that
    /// do not pitch-shift at all (`Off`, `Robot`). Female/Male aim
    /// beyond a subtle falsetto: the formants track the pitch, so the
    /// vocal tract itself sounds smaller/larger.
    pub fn default_pitch(self) -> Option<f32> {
        match self {
            VoicePreset::Mickey => Some(7.0),
            VoicePreset::Female => Some(6.0),
            VoicePreset::Male => Some(-5.0),
            VoicePreset::Off | VoicePreset::Robot => None,
        }
    }

    /// Which DSP backend implements this preset. `None` → no plugin
    /// needed (preset `Off`).
    pub fn required_plugin(self) -> Option<LadspaPlugin> {
        match self {
            VoicePreset::Off => None,
            // Robotization is ring modulation, not pitch shifting.
            VoicePreset::Robot => Some(LadspaPlugin::RingMod),
            _ => Some(LadspaPlugin::PitchShifter),
        }
    }
}

/// External LADSPA plugin .so basenames the mic chain can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LadspaPlugin {
    /// `am_pitchshift_1433.so` — swh pitch shifter (preferred: proven
    /// to process audio in the filter-graph on this stack).
    AmPitch,
    /// `ladspa-rubberband.so` — quality pitch shifter.
    PitchShifter,
    /// `pitch_scale_1193.so` — swh fallback pitch scaler.
    PitchScaler,
    /// `ringmod_1188.so` — swh ring modulator (robot voice).
    RingMod,
    /// `gate_1410.so` — swh noise gate (denoise fallback).
    Gate,
    /// `librnnoise_ladspa.so` — werman noise-suppression-for-voice:
    /// real rnnoise suppression, preferred over the gate.
    Rnnoise,
    /// `mbeq_1197.so` — swh multiband EQ, shapes the vocal timbre
    /// behind the pitch shifter (mono, same proven topology).
    Mbeq,
}

impl LadspaPlugin {
    /// Shared object basename without `.so` (as used in filter-chain
    /// `plugin = "..."`).
    pub fn so_name(self) -> &'static str {
        match self {
            LadspaPlugin::AmPitch => "am_pitchshift_1433",
            LadspaPlugin::PitchShifter => "ladspa-rubberband",
            LadspaPlugin::PitchScaler => "pitch_scale_1193",
            LadspaPlugin::RingMod => "ringmod_1188",
            LadspaPlugin::Gate => "gate_1410",
            LadspaPlugin::Rnnoise => "librnnoise_ladspa",
            LadspaPlugin::Mbeq => "mbeq_1197",
        }
    }
}

/// Commands the GUI sends into the PipeWire worker thread.
#[derive(Debug)]
pub enum BackendCommand {
    /// Set volume on a node (cubic scale, clamped to `0.0..=MAX_VOLUME`).
    SetVolume { id: u32, volume: f32 },
    /// Set mute on a node.
    SetMute { id: u32, muted: bool },
    /// Make the named node the session default for its direction.
    SetDefault { name: String, direction: Direction },
    /// Enable/disable the built-in effects chain (loads/unloads the
    /// filter-chain modules and reroutes the default sink).
    SetFxEnabled(bool),
    /// Live-update an effect's control value (slider units).
    SetEffectValue { effect: FxEffect, value: f32 },
    /// Enable/disable the microphone effects chain — the last stage
    /// presents `skald.mic.out` as a virtual `Audio/Source` that
    /// becomes the session default input.
    SetMicFxEnabled(bool),
    /// Rebuild the voice stage with a different preset graph.
    SetVoicePreset(VoicePreset),
    /// Toggle the noise-gate stage inside the mic chain (rebuild).
    SetMicGate(bool),
    /// Live-update a mic effect control value (slider units).
    SetMicEffectValue { effect: MicEffect, value: f32 },
    /// Loop the (processed) microphone back to the current default
    /// sink so the user can hear the applied effects — GUI-scoped
    /// module that dies with the window to avoid stale feedback.
    SetMicMonitor(bool),
    /// Point the microphone level-meter stream at a source node id.
    SetMeterTarget(Option<u32>),
    /// Stop the main loop and let the worker thread exit.
    Quit,
}

/// Maximum volume the UI/backend will apply (150 %, boost range).
pub const MAX_VOLUME: f32 = 1.5;

/// Events pushed from the worker thread to the GUI.
#[derive(Debug)]
pub enum BackendEvent {
    /// The complete endpoint list changed (node added or removed).
    EndpointsChanged(Vec<AudioEndpoint>),
    /// The app-stream list changed — a stream started or stopped.
    /// Only `Stream/Output/Audio` nodes are tracked; input-direction
    /// app streams are not (the mic side has its own meter).
    StreamsChanged(Vec<AppStream>),
    /// A single stream's volume/mute changed.
    StreamChanged(AppStream),
    /// A single endpoint's volume/mute/default state changed.
    EndpointChanged(AudioEndpoint),
    /// The built-in output effects chain was enabled/disabled — driven
    /// by the presence of `skald.fx.*` nodes in the registry, so
    /// the GUI reflects chains owned by the background daemon too.
    FxStateChanged(bool),
    /// The microphone effects chain appeared/disappeared in the
    /// registry (final source `skald.mic.out`).
    MicFxStateChanged(bool),
    /// Live microphone level update (0.0..=1.0 peak) for the
    /// input-activity indicator.
    MicLevel(f32),
    /// The PipeWire daemon went away or the connection failed.
    BackendLost(String),
}
