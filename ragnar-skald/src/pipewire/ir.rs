// Copyright (C) Jörg Schröder
//! Synthetic impulse response for the built-in reverb (convolver).
//!
//! PipeWire's `convolver` filter loads an impulse response through
//! libsndfile, which needs a real audio file on disk. Instead of
//! shipping a binary blob in the repository we synthesize a short
//! exponentially-decaying stereo noise IR at runtime — that is exactly
//! what a hall reverb tail looks like acoustically.
//!
//! Written to `$XDG_STATE_HOME/ragnar-skald/reverb-ir.wav` (or
//! `~/.local/state/ragnar-skald/`), generated once and reused.

use std::f32::consts::PI;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use tracing::debug;

/// Sample rate the convolver runs at (PipeWire graph rate).
const IR_RATE: u32 = 48_000;
/// Tail length in seconds — short hall, keeps CPU cost low.
const IR_SECONDS: f32 = 0.8;

/// Return the IR file path, generating the wav on first use.
pub fn impulse_response_path() -> Result<PathBuf> {
    let dir = dirs::state_dir()
        .or_else(dirs::data_dir)
        .map(|d| d.join("ragnar-skald"))
        .context("no XDG state/data directory available")?;
    fs::create_dir_all(&dir).context("failed to create Skald state dir")?;
    let path = dir.join("reverb-ir.wav");
    if !path.exists() {
        write_ir(&path).context("failed to write reverb impulse response")?;
        debug!("Generated reverb impulse response at {path:?}");
    }
    Ok(path)
}

/// Synthesize the IR: two decorrelated noise tails with exponential
/// decay — L/R use different seeds for stereo width.
fn synth_frame(i: usize) -> (f32, f32) {
    let t = i as f32 / IR_RATE as f32;
    // 4-fold exponential decay ~ -60 dB at the end of the tail.
    let decay = (-t * 9.0 / IR_SECONDS).exp();
    // Deterministic pseudo-noise (xorshift-style mixing) — reproducible
    // so a regenerate is byte-identical.
    let n = i as u32;
    let l = noise(n ^ 0x9e37_79b9);
    let r = noise(n.wrapping_mul(0x85eb_ca6b) ^ 0xc2b2_ae35);
    // Slight lowpass character: darken towards the end.
    let tone = 0.55 + 0.45 * (t * 12.0 * PI / IR_SECONDS).cos();
    (l * decay * tone, r * decay * tone)
}

/// Hash-to-noise in [-1, 1] — a cheap deterministic decorrelator.
fn noise(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Write a 16-bit PCM stereo wav of the synthesized IR.
fn write_ir(path: &PathBuf) -> io::Result<()> {
    let frames = (IR_SECONDS * IR_RATE as f32) as usize;
    let data_len = frames * 2 * 2; // stereo * s16
    let mut f = io::BufWriter::new(fs::File::create(path)?);

    // Minimal RIFF/WAVE header.
    f.write_all(b"RIFF")?;
    f.write_all(&(36u32 + data_len as u32).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?; // fmt chunk size
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&2u16.to_le_bytes())?; // channels
    f.write_all(&IR_RATE.to_le_bytes())?;
    f.write_all(&(IR_RATE * 2 * 2).to_le_bytes())?; // byte rate
    f.write_all(&4u16.to_le_bytes())?; // block align
    f.write_all(&16u16.to_le_bytes())?; // bits
    f.write_all(b"data")?;
    f.write_all(&(data_len as u32).to_le_bytes())?;

    for i in 0..frames {
        let (l, r) = synth_frame(i);
        f.write_all(&((l * i16::MAX as f32).clamp(-32768.0, 32767.0) as i16).to_le_bytes())?;
        f.write_all(&((r * i16::MAX as f32).clamp(-32768.0, 32767.0) as i16).to_le_bytes())?;
    }
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ir_decays_and_is_bounded() {
        let (l0, r0) = synth_frame(0);
        let (l_end, _) = synth_frame(38399);
        assert!(l0.abs() <= 1.0 && r0.abs() <= 1.0);
        assert!(l_end.abs() < l0.abs() * 0.05, "tail must decay");
    }

    #[test]
    fn ir_channels_are_decorrelated() {
        let (l, r) = synth_frame(1234);
        assert_ne!(l, r, "L/R must differ for stereo width");
    }
}
