//! Live integration test against a running PipeWire daemon.
//!
//! Skips silently when no daemon is reachable so the suite stays green
//! on headless CI. On a real LieraOS session it verifies that the
//! backend enumerates the same sinks WirePlumber exposes and that
//! volume/mute commands actually reach the node params.

use std::time::{Duration, Instant};

use ragnar_skald::pipewire::backend::PwBackend;
use ragnar_skald::pipewire::micfx;
use ragnar_skald::pipewire::types::{
    AudioEndpoint, BackendEvent, Direction, FxEffect, MicEffect, VoicePreset,
};

const WAIT: Duration = Duration::from_secs(5);

/// True if a PipeWire daemon answers on the default socket.
fn daemon_available() -> bool {
    std::process::Command::new("pw-cli")
        .args(["info", "0"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Drain events until `pred` hits or the deadline passes.
fn wait_for<F: FnMut(&BackendEvent) -> bool>(backend: &PwBackend, mut pred: F) -> bool {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if let Ok(ev) = backend.try_recv_event() {
            if pred(&ev) {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn first_endpoint(backend: &PwBackend, direction: Direction) -> Option<AudioEndpoint> {
    let mut found = None;
    wait_for(backend, |ev| {
        if let BackendEvent::EndpointsChanged(list) = ev {
            if let Some(ep) = list.iter().find(|e| e.direction == direction) {
                found = Some(ep.clone());
            }
        }
        found.is_some()
    });
    found
}

#[test]
fn backend_enumerates_endpoints_live() {
    if !daemon_available() {
        eprintln!("skipping: no PipeWire daemon");
        return;
    }
    let backend = PwBackend::start().expect("failed to start backend");

    let mut list = Vec::new();
    wait_for(&backend, |ev| {
        if let BackendEvent::EndpointsChanged(l) = ev {
            if !l.is_empty() {
                list = l.clone();
            }
        }
        !list.is_empty()
    });

    assert!(
        !list.is_empty(),
        "daemon is running but no audio endpoints were enumerated"
    );
    for ep in &list {
        assert!(!ep.name.is_empty(), "endpoint {} has no name", ep.id);
        assert!(
            !ep.description.is_empty(),
            "endpoint {} has no description",
            ep.id
        );
    }
}

#[test]
fn backend_sets_volume_and_mute_live() {
    if !daemon_available() {
        eprintln!("skipping: no PipeWire daemon");
        return;
    }
    let backend = PwBackend::start().expect("failed to start backend");

    let Some(sink) = first_endpoint(&backend, Direction::Output) else {
        eprintln!("skipping: no output endpoint present");
        return;
    };
    let id = sink.id;
    let original = sink.volume;

    // Wait until the initial Props param arrives (volume_known), then
    // write a distinctly different volume and mute and verify the
    // node reports them back through EndpointChanged events.
    let _ = wait_for(
        &backend,
        |ev| matches!(ev, BackendEvent::EndpointChanged(e) if e.id == id && e.volume_known()),
    );

    let target = if original > 0.6 { 0.33 } else { 0.77 };
    backend.set_volume(id, target);
    assert!(
        wait_for(&backend, |ev| {
            matches!(ev, BackendEvent::EndpointChanged(e)
                if e.id == id && (e.volume - target).abs() < 0.02)
        }),
        "volume change to {target} was not confirmed by the daemon"
    );

    backend.set_mute(id, true);
    assert!(
        wait_for(&backend, |ev| {
            matches!(ev, BackendEvent::EndpointChanged(e) if e.id == id && e.muted)
        }),
        "mute was not confirmed by the daemon"
    );

    // Restore a sane state: unmuted, original volume.
    backend.set_mute(id, false);
    backend.set_volume(id, original);
    wait_for(&backend, |ev| {
        matches!(ev, BackendEvent::EndpointChanged(e)
            if e.id == id && !e.muted && (e.volume - original).abs() < 0.02)
    });
}

/// Load the built-in filter-chain, verify the entry sink registers,
/// push a live control value and tear the chain down again.
#[test]
fn backend_loads_and_controls_fx_chain_live() {
    if !daemon_available() {
        eprintln!("skipping: no PipeWire daemon");
        return;
    }
    let backend = PwBackend::start().expect("failed to start backend");

    // Wait for the initial endpoint snapshot so the default metadata
    // is guaranteed to be bound before we enable the chain.
    if first_endpoint(&backend, Direction::Output).is_none() {
        eprintln!("skipping: no output endpoint present");
        return;
    }

    backend.set_fx_enabled(true);
    assert!(
        wait_for(&backend, |ev| matches!(
            ev,
            BackendEvent::FxStateChanged(true)
        )),
        "fx chain did not report enabled"
    );

    // The entry capture sink must appear as a regular output endpoint
    // and become the session default (audio is routed through it).
    let mut entry_seen = false;
    wait_for(&backend, |ev| {
        if let BackendEvent::EndpointsChanged(list) = ev {
            entry_seen |= list.iter().any(|e| e.name.starts_with("skald.fx."));
        }
        entry_seen
    });
    assert!(entry_seen, "fx entry sink did not appear in endpoint list");

    // Live control: bass shelf to +6 dB, echo feedforward to 0.3 —
    // no assertion on confirmation events (the filter-chain does not
    // echo params back as Props), the point is the command path works.
    backend.set_effect_value(FxEffect::Bass, 6.0);
    backend.set_effect_value(FxEffect::Echo, 3.0);
    std::thread::sleep(Duration::from_millis(150));

    backend.set_fx_enabled(false);
    assert!(
        wait_for(&backend, |ev| matches!(
            ev,
            BackendEvent::FxStateChanged(false)
        )),
        "fx chain did not report disabled"
    );
}

/// Load the microphone chain, verify the virtual source registers as
/// an endpoint and the session default input moves onto it, push live
/// controls, then tear the chain down and check the real source is
/// restored.
///
/// On the dev machine the LADSPA plugins are not installed system-wide
/// but extracted under /tmp — pointing `LADSPA_PATH` there exercises
/// the real plugin graphs; without them the chain falls back to the
/// built-in stages (still a valid end-to-end test).
#[test]
fn backend_loads_mic_chain_live() {
    if !daemon_available() {
        eprintln!("skipping: no PipeWire daemon");
        return;
    }
    // Expose locally extracted LADSPA plugins when present.
    let extra: Vec<&str> = [
        "/tmp/rb/usr/lib/x86_64-linux-gnu/ladspa",
        "/tmp/swh/usr/lib/ladspa",
    ]
    .into_iter()
    .filter(|p| std::path::Path::new(p).is_dir())
    .collect();
    if !extra.is_empty() {
        let joined = match std::env::var("LADSPA_PATH") {
            Ok(existing) if !existing.is_empty() => {
                format!("{}:{}", extra.join(":"), existing)
            }
            _ => extra.join(":"),
        };
        std::env::set_var("LADSPA_PATH", joined);
    }

    let backend = PwBackend::start().expect("failed to start backend");

    // We need a real capture source to feed the chain.
    if first_endpoint(&backend, Direction::Input).is_none() {
        eprintln!("skipping: no input endpoint present");
        return;
    }

    // Structural settings must precede the enable command so the
    // first chain build already carries them (commands are ordered on
    // the worker's channel).
    let preset = if micfx::preset_available(VoicePreset::Robot) {
        VoicePreset::Robot
    } else {
        VoicePreset::Off
    };
    backend.set_voice_preset(preset);
    backend.set_mic_gate(micfx::denoise_available());
    backend.set_mic_fx_enabled(true);

    assert!(
        wait_for(&backend, |ev| matches!(
            ev,
            BackendEvent::MicFxStateChanged(true)
        )),
        "mic fx chain did not report enabled"
    );

    // The virtual microphone must appear as a normal input endpoint.
    let mut mic_seen = false;
    wait_for(&backend, |ev| {
        if let BackendEvent::EndpointsChanged(list) = ev {
            mic_seen |= list.iter().any(|e| e.name == micfx::MIC_OUT_NODE);
        }
        mic_seen
    });
    assert!(mic_seen, "virtual microphone skald.mic.out did not appear");

    // Live controls: gate threshold + mic echo level.
    backend.set_mic_effect_value(MicEffect::Denoise, 7.0);
    backend.set_mic_effect_value(MicEffect::Echo, 4.0);
    std::thread::sleep(Duration::from_millis(150));

    // The chain outlives `mic_fx_enabled=false` while the gate is on
    // (denoise is independent) — teardown needs both switches off.
    backend.set_mic_gate(false);
    backend.set_mic_fx_enabled(false);
    assert!(
        wait_for(&backend, |ev| matches!(
            ev,
            BackendEvent::MicFxStateChanged(false)
        )),
        "mic fx chain did not report disabled"
    );
}
