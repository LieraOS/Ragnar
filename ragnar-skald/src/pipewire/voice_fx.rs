// Copyright (C) Jörg Schröder
//! Voice-preset filter graphs for the mic chain.
//!
//! Each preset maps to a `filter.graph` node list. Pitch presets
//! (Mickey/Female/Male) append a static timbre EQ after the pitch
//! node — the "Klangfarbe" that makes gender voices convincing: a
//! bare pitch shift sounds like falsetto, while EQ that tracks the
//! voice direction shapes the perceived vocal tract (brighter and
//! lighter for Female, warmer and heavier for Male).
//!
//! The pitch node always stays FIRST in the graph: PipeWire routes
//! runtime `params` to the first node, which is what keeps its
//! semitone control ("Pitch shift"/"Semitones") live-adjustable
//! from the GUI pitch slider.

use super::micfx;
use super::types::{LadspaPlugin, VoicePreset};

/// Plain pass-through used when a preset's plugin is missing — the
/// stage stays in the chain so topology and control wiring never
/// depend on which plugins are installed.
const PASSTHROUGH: &str = "{ nodes = [ { type = builtin name = fx label = copy } ] }";

/// Voice transformation graph for `preset`, or `copy` passthrough
/// when the required plugin is unavailable (the module still loads —
/// the preset just has no audible effect and the GUI can show the
/// plugin as missing).
pub fn graph(preset: VoicePreset, pitch: Option<LadspaPlugin>) -> String {
    match preset {
        VoicePreset::Off => String::from(PASSTHROUGH),
        VoicePreset::Robot => robot_graph(),
        _ => pitch_graph(preset, pitch),
    }
}

/// Dalek-style robot voice: swh ring modulator, RM depth 2.0 + a
/// low 55 Hz LFO sine keeps speech rhythm recognizable.
fn robot_graph() -> String {
    if micfx::ladspa_path(LadspaPlugin::RingMod).is_none() {
        warn_missing(VoicePreset::Robot, LadspaPlugin::RingMod);
        return String::from(PASSTHROUGH);
    }
    micfx::graph_of(&micfx::ladspa_spec(
        LadspaPlugin::RingMod,
        "ringmod_1i1o1l",
        r#""Modulation depth (0=none, 1=AM, 2=RM)" = 2.0
           "Frequency (Hz)" = 55.0
           "Sine level" = 1.0
           "Triangle level" = 0.0
           "Sawtooth level" = 0.0
           "Square level" = 0.0"#,
    ))
}

/// Pitch-shifted voice with the preset's base semitone shift baked
/// in — the GUI slider retunes it live afterwards. Female/Male add
/// a static EQ node pair after the pitch shifter for timbre.
fn pitch_graph(preset: VoicePreset, pitch: Option<LadspaPlugin>) -> String {
    let Some(base) = preset.default_pitch() else {
        return String::from(PASSTHROUGH);
    };
    let Some(pitch) = pitch else {
        warn_missing(preset, LadspaPlugin::PitchShifter);
        return String::from(PASSTHROUGH);
    };

    // Pitch node first — runtime params reach the first node only.
    let pitch_node = match pitch {
        // swh am_pitchshift — mono node duplicated per channel, same
        // proven topology as gate/ringmod. "Pitch shift" takes
        // semitones directly and is live-controllable.
        LadspaPlugin::AmPitch => micfx::ladspa_spec(
            LadspaPlugin::AmPitch,
            "amPitchshift",
            &format!(r#""Pitch shift" = {base}"#),
        ),
        LadspaPlugin::PitchShifter => {
            // Formant preserving keeps the vocal character for the
            // gender presets; Mickey wants the raw chipmunk shift.
            let formant = if preset == VoicePreset::Mickey {
                0.0
            } else {
                1.0
            };
            micfx::ladspa_spec(
                LadspaPlugin::PitchShifter,
                "rubberband-live-pitchshifter-mono",
                &format!(
                    r#""Semitones" = {base}
                       "Formant Preserving" = {formant}
                       "Wet-Dry Mix" = 1.0"#
                ),
            )
        }
        // swh pitchScale is a crude varispeed scaler — takes a ratio
        // instead of semitones, so it gets the converted base and no
        // live slider.
        _ => {
            let coeff = 2.0f32.powf(base / 12.0);
            micfx::ladspa_spec(
                LadspaPlugin::PitchScaler,
                "pitchScale",
                &format!(r#""Pitch co-efficient" = {coeff:.3}"#),
            )
        }
    };

    // Multi-node graphs need explicit port wiring: `links` chains
    // pitch output → EQ input, `inputs`/`outputs` declare the graph's
    // boundary ports. Without them the filter produces silence —
    // "links can be omitted when the graph has just 1 filter".
    let eq = timbre_eq(preset);
    if eq.is_empty() {
        micfx::graph_of(&pitch_node)
    } else {
        format!(
            "{{ nodes = [ {pitch_node} {eq} ] \
                links = [ {{ output = \"fx:Output\" input = \"fx_eq:Input\" }} ] \
                inputs = [ \"fx:Input\" ] \
                outputs = [ \"fx_eq:Output\" ] }}"
        )
    }
}

/// Static timbre EQ appended after the pitch node — a second mono
/// LADSPA stage (`mbeq` multiband EQ), not a builtin filter: mixing
/// builtin `bq_*` nodes after a duplicated mono plugin leaves the
/// chain silent (port/channel wiring the graph engine cannot bridge).
///
/// Female: lighter body (cut lows) plus presence lift (boost highs).
/// Male: added chest warmth (boost lows) plus softened sibilance
/// (cut highs). Missing `mbeq` simply skips the shaping — pitch
/// alone still works.
fn timbre_eq(preset: VoicePreset) -> String {
    if micfx::ladspa_path(LadspaPlugin::Mbeq).is_none() {
        return String::new();
    }
    let control = match preset {
        VoicePreset::Female => {
            r#""50Hz gain (low shelving)" = -6.0
               "100Hz gain" = -4.0
               "156Hz gain" = -2.0
               "3500Hz gain" = 3.0
               "5000Hz gain" = 3.0
               "10000Hz gain" = 2.0"#
        }
        VoicePreset::Male => {
            r#""50Hz gain (low shelving)" = 2.0
               "100Hz gain" = 3.0
               "156Hz gain" = 3.0
               "5000Hz gain" = -2.0
               "10000Hz gain" = -3.0"#
        }
        _ => return String::new(),
    };
    format!(
        " {}",
        micfx::ladspa_spec_named(LadspaPlugin::Mbeq, "mbeq", control, "fx_eq")
    )
}

/// Log once per missing plugin so a broken chain never silently
/// degrades — the voice stage runs as pass-through instead.
fn warn_missing(preset: VoicePreset, plugin: LadspaPlugin) {
    tracing::warn!(
        "Voice preset {preset:?}: plugin {:?} not installed — stage runs as pass-through",
        plugin.so_name()
    );
}
