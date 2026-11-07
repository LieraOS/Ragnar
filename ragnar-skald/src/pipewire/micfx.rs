//! Microphone (input-direction) filter-chain effects.
//!
//! Mirror image of [`super::fxchain`]: instead of presenting a sink
//! that apps play into, the chain presents a virtual **source**
//! (`skald.mic.out`) that apps record from. Each stage's capture
//! stream is pinned to the previous stage's output via `target.object`,
//! so the signal path is:
//!
//! ```text
//! real mic → denoise.in ─→ denoise.out ─→ voice.in ─→ voice.out
//!         → echo.in  ─→ echo.out (= skald.mic.out, Audio/Source)
//!         → apps
//! ```
//!
//! The final playback node always keeps the stable name
//! `skald.mic.out`, no matter which stages are active — the
//! session default, the preview loopback and external apps can rely on
//! it. Intermediate `*.out` nodes are `Audio/Source`s as well (a
//! capture stream can only attach to a source node); our own UI hides
//! everything under [`MICFX_NODE_PREFIX`].
//!
//! LADSPA plugins are optional capabilities: each stage/preset knows
//! which `.so` it needs and [`ladspa_path`] resolves the real file,
//! honouring `LADSPA_PATH` like PipeWire's own loader does.

use std::path::{Path, PathBuf};

use super::fxchain;
use super::types::{LadspaPlugin, MicEffect, VoicePreset};

/// Node name prefix for internal microphone-chain nodes.
pub const MICFX_NODE_PREFIX: &str = "skald.micfx.";

/// Stable name of the final virtual microphone the chain presents.
pub const MIC_OUT_NODE: &str = "skald.mic.out";

/// `true` when a node name belongs to the microphone chain.
pub fn is_micfx_node(name: &str) -> bool {
    name.starts_with(MICFX_NODE_PREFIX)
}

/// Map a mic-chain capture node name back to its stage.
pub fn effect_for_capture(name: &str) -> Option<MicEffect> {
    let key = name.strip_prefix(MICFX_NODE_PREFIX)?.strip_suffix(".in")?;
    MicEffect::from_key(key)
}

/// Signal order through the chain: cleanup first, character second,
/// ambience last (echo feeds the virtual mic).
pub const CHAIN_ORDER: &[MicEffect] = &[MicEffect::Denoise, MicEffect::Voice, MicEffect::Echo];

/// Resolve a LADSPA plugin basename to an existing `.so` file.
///
/// Mirrors PipeWire's own search order exactly: `LADSPA_PATH` first,
/// then the classic system directories including the Debian
/// multiarch dir. User dirs like `~/.ladspa` are deliberately NOT
/// searched — filter-chain only loads plugins located inside its own
/// search path, so resolving one there would name a graph PipeWire
/// cannot actually load.
pub fn ladspa_path(plugin: LadspaPlugin) -> Option<PathBuf> {
    let file = format!("{}.so", plugin.so_name());
    let mut dirs: Vec<PathBuf> = std::env::var_os("LADSPA_PATH")
        .map(|v| std::env::split_paths(&v).collect())
        .unwrap_or_default();
    dirs.extend(
        [
            "/usr/lib64/ladspa",
            "/usr/lib/ladspa",
            "/usr/lib/x86_64-linux-gnu/ladspa",
        ]
        .iter()
        .map(PathBuf::from),
    );
    dirs.into_iter().map(|d| d.join(&file)).find(|p| p.exists())
}

/// Best pitch-shifting plugin available on this system.
pub fn pitch_plugin() -> Option<LadspaPlugin> {
    // swh am_pitchshift first — it demonstrably processes audio in the
    // filter-graph (same plugin family as the working gate/ringmod);
    // rubberband's ladspa output stayed silent on this stack.
    if ladspa_path(LadspaPlugin::AmPitch).is_some() {
        Some(LadspaPlugin::AmPitch)
    } else if ladspa_path(LadspaPlugin::PitchShifter).is_some() {
        Some(LadspaPlugin::PitchShifter)
    } else {
        ladspa_path(LadspaPlugin::PitchScaler).map(|_| LadspaPlugin::PitchScaler)
    }
}

/// Whether the given voice preset can actually be produced on this
/// system (its required plugin is installed).
pub fn preset_available(preset: VoicePreset) -> bool {
    match preset.required_plugin() {
        None => true,
        Some(LadspaPlugin::PitchShifter) => pitch_plugin().is_some(),
        Some(other) => ladspa_path(other).is_some(),
    }
}

/// Denoise plugin to use: rnnoise (`librnnoise_ladspa`) performs real
/// spectral suppression and is preferred; the swh `gate` is the
/// fallback for systems without the rnnoise plugin.
pub fn denoise_plugin() -> Option<LadspaPlugin> {
    if ladspa_path(LadspaPlugin::Rnnoise).is_some() {
        Some(LadspaPlugin::Rnnoise)
    } else {
        ladspa_path(LadspaPlugin::Gate).map(|_| LadspaPlugin::Gate)
    }
}

/// Whether the denoise stage can run (rnnoise or gate installed).
pub fn denoise_available() -> bool {
    denoise_plugin().is_some()
}

/// Runtime slider mapping for a mic effect, resolved against the
/// plugin the stage actually loads. Denoise differs per plugin:
/// rnnoise exposes "VAD Threshold (%)" (0..100), the gate exposes
/// "Threshold (dB)" (-55..-15 over the 0..10 slider). Voice maps to
/// the pitch shifter's semitone control — only for pitch presets,
/// and only when the available shifter speaks semitones (the
/// pitchScale fallback takes a ratio instead and gets no slider).
pub fn effect_control(effect: MicEffect, preset: VoicePreset) -> Option<(&'static str, f32, f32)> {
    match effect {
        MicEffect::Denoise => denoise_plugin().map(|p| match p {
            // VAD Threshold % — higher = stricter speech requirement.
            LadspaPlugin::Rnnoise => ("VAD Threshold (%)", 10.0, 0.0),
            _ => ("Threshold (dB)", 4.0, -55.0),
        }),
        MicEffect::Voice => {
            preset.default_pitch()?;
            pitch_plugin().and_then(|p| match p {
                LadspaPlugin::AmPitch => Some(("Pitch shift", 1.0, 0.0)),
                LadspaPlugin::PitchShifter => Some(("Semitones", 1.0, 0.0)),
                _ => None,
            })
        }
        other => other.control(),
    }
}

/// Build the module `args` string (SPA-JSON) for one mic-chain stage.
///
/// `upstream` is the node name the capture stream attaches to: the real
/// microphone for the first stage, the previous stage's `*.out` source
/// afterwards. `is_last` decides whether the playback node gets the
/// stable [`MIC_OUT_NODE`] name.
pub fn module_args(
    effect: MicEffect,
    upstream: &str,
    is_last: bool,
    preset: VoicePreset,
    pitch: Option<LadspaPlugin>,
) -> String {
    let key = effect.key();
    let out_name = if is_last {
        MIC_OUT_NODE.to_string()
    } else {
        format!("{MICFX_NODE_PREFIX}{key}.out")
    };
    let out_desc = if is_last {
        "Skald Mikrofon".to_string()
    } else {
        format!("Skald Mikrofon ({})", effect.label())
    };
    let graph = graph_config(effect, preset, pitch);

    format!(
        "{{ node.description = \"Skald Mic: {label}\" \
            filter.graph = {graph} \
            audio.channels = 2 \
            audio.position = [ FL FR ] \
            capture.props = {{ \
                node.name = \"{prefix}{key}.in\" \
                node.passive = true \
                target.object = \"{upstream}\" \
            }} \
            playback.props = {{ \
                node.name = \"{out_name}\" \
                node.description = \"{out_desc}\" \
                media.class = Audio/Source \
            }} \
        }}",
        label = effect.label(),
        graph = graph,
        prefix = MICFX_NODE_PREFIX,
        key = key,
        upstream = upstream,
        out_name = out_name,
        out_desc = out_desc,
    )
}

/// `filter.graph` node config per mic stage.
fn graph_config(effect: MicEffect, preset: VoicePreset, pitch: Option<LadspaPlugin>) -> String {
    match effect {
        // rnnoise first — real spectral suppression (the PipeWire
        // source-rnnoise.conf template uses the same stereo label and
        // "VAD Threshold (%)" control). The swh gate remains as the
        // fallback when the plugin is not installed.
        MicEffect::Denoise => match denoise_plugin() {
            Some(LadspaPlugin::Rnnoise) => ladspa_node(
                LadspaPlugin::Rnnoise,
                "noise_suppressor_stereo",
                r#""VAD Threshold (%)" = 50.0"#,
            ),
            _ => ladspa_node(
                LadspaPlugin::Gate,
                "gate",
                r#""LF key filter (Hz)" = 50.0
                   "HF key filter (Hz)" = 8000.0
                   "Threshold (dB)" = -45.0
                   "Attack (ms)" = 15.0
                   "Hold (ms)" = 120.0
                   "Decay (ms)" = 400.0
                   "Range (dB)" = -55.0
                   "Output select (-1 = key listen, 0 = gate, 1 = bypass)" = 0.0"#,
            ),
        },
        MicEffect::Voice => super::voice_fx::graph(preset, pitch),
        // Same wet/dry topology as the output echo — `fx_mix` stays the
        // first node so `Gain 2` is the live-controllable intensity.
        MicEffect::Echo => fxchain::echo_graph(),
        // Bare copy node — the virtual mic exists and routes audio but
        // applies zero processing while every effect stage is off.
        MicEffect::Pass => graph_of("{ type = builtin name = fx label = copy }"),
    }
}

/// Single mono LADSPA node graph — the engine duplicates per channel.
fn ladspa_node(plugin: LadspaPlugin, label: &str, control: &str) -> String {
    graph_of(&ladspa_spec(plugin, label, control))
}

/// Bare LADSPA node spec (without the `nodes = […]` wrapper) — shared
/// with `voice_fx`, which composes multi-node graphs. `name` must be
/// unique inside a graph; the FIRST node keeps `fx` because runtime
/// `params` are routed there.
pub(crate) fn ladspa_spec(plugin: LadspaPlugin, label: &str, control: &str) -> String {
    ladspa_spec_named(plugin, label, control, "fx")
}

/// Named variant of [`ladspa_spec`] for multi-node graphs.
pub(crate) fn ladspa_spec_named(
    plugin: LadspaPlugin,
    label: &str,
    control: &str,
    name: &str,
) -> String {
    let path = ladspa_path(plugin)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| plugin.so_name().to_string());
    format!(
        "{{ type = ladspa name = {name} plugin = \"{path}\" \
            label = \"{label}\" control = {{ {control} }} }}"
    )
}

/// Wrap node specs into a `filter.graph` value — nodes chain in
/// listed order (capture → first → … → playback).
pub(crate) fn graph_of(nodes: &str) -> String {
    format!("{{ nodes = [ {nodes} ] }}")
}

/// Convenience used by tests: does a path look like a LADSPA dir entry?
#[allow(dead_code)]
fn is_ladspa_file(path: &Path) -> bool {
    path.extension().map(|e| e == "so").unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn micfx_node_prefix_matching() {
        assert!(is_micfx_node("skald.micfx.denoise.in"));
        assert!(is_micfx_node("skald.micfx.echo.out"));
        // The stable virtual mic deliberately lives outside the prefix
        // so it shows up as a normal endpoint.
        assert!(!is_micfx_node(MIC_OUT_NODE));
        assert!(!is_micfx_node("alsa_input.pci-0"));
    }

    #[test]
    fn capture_name_roundtrips_effect() {
        for effect in CHAIN_ORDER {
            let name = format!("{MICFX_NODE_PREFIX}{}.in", effect.key());
            assert_eq!(effect_for_capture(&name), Some(*effect));
        }
        assert_eq!(effect_for_capture("skald.micfx.voice.out"), None);
        assert_eq!(effect_for_capture("other.node.in"), None);
    }

    #[test]
    fn module_args_targets_upstream_and_names_nodes() {
        let args = module_args(
            MicEffect::Echo,
            "alsa_input.usb-mic",
            false,
            VoicePreset::Off,
            None,
        );
        assert!(args.contains("target.object = \"alsa_input.usb-mic\""));
        assert!(args.contains("node.name = \"skald.micfx.echo.in\""));
        assert!(args.contains("node.name = \"skald.micfx.echo.out\""));
        assert!(args.contains("media.class = Audio/Source"));
    }

    #[test]
    fn last_stage_gets_stable_virtual_mic_name() {
        let args = module_args(MicEffect::Echo, "upstream", true, VoicePreset::Off, None);
        assert!(args.contains("node.name = \"skald.mic.out\""));
    }

    #[test]
    fn off_preset_builds_passthrough_graph() {
        let args = module_args(MicEffect::Voice, "mic", true, VoicePreset::Off, None);
        assert!(args.contains("label = copy"));
        assert!(!args.contains("type = ladspa"));
    }

    #[test]
    fn pitch_preset_without_plugin_falls_back_to_passthrough() {
        // `pitch = None` models "neither rubberband nor pitchScale
        // installed" — the graph must stay loadable.
        let args = module_args(MicEffect::Voice, "mic", true, VoicePreset::Mickey, None);
        assert!(args.contains("label = copy"));
    }

    #[test]
    fn effect_control_mappings_match_plugin_ranges() {
        // Denoise resolves per installed plugin: rnnoise → VAD %,
        // gate → dB threshold, neither → no control.
        match denoise_plugin() {
            Some(LadspaPlugin::Rnnoise) => {
                let (name, scale, offset) =
                    effect_control(MicEffect::Denoise, VoicePreset::Off).unwrap();
                assert_eq!(name, "VAD Threshold (%)");
                assert_eq!(offset, 0.0);
                assert_eq!(offset + 10.0 * scale, 100.0);
            }
            Some(_) => {
                let (name, scale, offset) =
                    effect_control(MicEffect::Denoise, VoicePreset::Off).unwrap();
                assert_eq!(name, "Threshold (dB)");
                assert_eq!(offset, -55.0);
                assert_eq!(offset + 10.0 * scale, -15.0);
            }
            None => assert!(effect_control(MicEffect::Denoise, VoicePreset::Off).is_none()),
        }
        // Echo: slider 0..10 → wet gain 0..0.5.
        let (name, scale, offset) = MicEffect::Echo.control().unwrap();
        assert_eq!(name, "Gain 2");
        assert_eq!(offset + 10.0 * scale, 0.5);
        // Voice has no runtime control — presets rebuild the graph.
        assert!(MicEffect::Voice.control().is_none());
    }

    #[test]
    fn preset_plugin_requirements() {
        assert_eq!(VoicePreset::Off.required_plugin(), None);
        assert_eq!(
            VoicePreset::Robot.required_plugin(),
            Some(LadspaPlugin::RingMod)
        );
        for p in [VoicePreset::Mickey, VoicePreset::Female, VoicePreset::Male] {
            assert_eq!(p.required_plugin(), Some(LadspaPlugin::PitchShifter));
        }
    }
}
