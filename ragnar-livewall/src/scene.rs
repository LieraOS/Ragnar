// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-livewall — scene assembly
//
// The wallpaper surface is a single ShaderWall widget that
// renders the picture plus the procedural GLSL aurora in one
// pass (see shaderwall.rs). On top sit the lamp overlays for
// the cabin's lit windows.
//
// A 66 ms tick advances the shader clock. When the fullscreen
// watcher reports a covered desktop the tick still runs but
// nothing is redrawn — zero GPU cost while a game owns the
// screen.

use gtk4::prelude::*;
use gtk4::{glib, Box, Fixed};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::glwall::GLWall;

/// Frame cadence — 15 fps is enough for a slow aurora.
const TICK_MS: u32 = 66;

/// Lit window in the picture, covered by two overlays: a warm
/// glow patch for the lamp shine and a dark patch for the rare
/// "light goes out" moments. Coordinates are fractions of the
/// surface and only valid for the stock aurora-fjord artwork —
/// build_lamps() therefore only attaches them for that file.
struct Lamp {
    glow: Box,
    dark: Box,
    /// Window rect as fractions of the surface.
    fx: f64,
    fy: f64,
    fw: f64,
    fh: f64,
    seed: u64,
    /// Remaining ticks of the current burst (0 = steady).
    burst: u32,
    /// Remaining ticks of a blackout inside the burst.
    out: u32,
}

/// Animated wallpaper state.
pub struct Scene {
    pub layer: Fixed,
    wall: GLWall,
    bg: gtk4::Picture,
    lamps: Vec<Lamp>,
    /// Shared pause flag — the watcher sets it, frame() reads.
    paused: Arc<AtomicBool>,
    tick: u64,
    w: i32,
    h: i32,
}

impl Scene {
    /// Build the layer: shader wall plus the lamp set.
    /// `paused` is flipped by the fullscreen watcher.
    pub fn new(image: &str, paused: Arc<AtomicBool>) -> Rc<RefCell<Self>> {
        let tex = gtk4::gdk::Texture::from_filename(image).unwrap_or_else(|_| {
            eprintln!("ragnar-livewall: cannot load {image}");
            empty_texture()
        });
        let wall = GLWall::new(&tex, crate::effects::load(std::path::Path::new(image)));
        // Static fallback under the GLArea: if GL init fails the
        // area hides and this picture carries the wallpaper.
        let bg = gtk4::Picture::for_paintable(&tex);
        bg.set_content_fit(gtk4::ContentFit::Cover);

        let layer = Fixed::new();
        layer.put(&bg, 0.0, 0.0);
        layer.put(&wall.area, 0.0, 0.0);

        // Cabin lamps — only meaningful on the stock artwork
        // where the window positions are known.
        let mut seed = 555777888u64;
        let lamps = build_lamps(image, &layer, &mut seed);

        let scene = Rc::new(RefCell::new(Self {
            layer,
            wall,
            bg,
            lamps,
            paused,
            tick: 0,
            w: 0,
            h: 0,
        }));

        let s = scene.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(TICK_MS as u64), move || {
            s.borrow_mut().frame();
            glib::ControlFlow::Continue
        });
        scene
    }

    /// One frame: relayout on size change, advance the shader
    /// clock, run the lamp state machines. When paused nothing
    /// redraws — the surface sleeps under the fullscreen app.
    fn frame(&mut self) {
        if self.paused.load(Ordering::Relaxed) {
            return;
        }
        self.tick += 1;
        let (w, h) = (self.layer.width(), self.layer.height());
        if w <= 0 {
            return;
        }
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.relayout();
        }
        // Seconds as f32 — plenty of precision for shader time.
        self.wall.set_time(self.tick as f32 * TICK_MS as f32 / 1000.0);
        self.lamp_frame();
    }

    /// Widget sizes for the current allocation: the wall spans
    /// everything, lamp patches sit on their windows.
    fn relayout(&mut self) {
        let (w, h) = (self.w, self.h);
        self.wall.area.set_size_request(w, h);
        self.bg.set_size_request(w, h);
        for l in &self.lamps {
            let (ww, wh) = (l.fw * w as f64, l.fh * h as f64);
            l.dark
                .set_size_request(ww.max(1.0) as i32, wh.max(1.0) as i32);
            l.glow
                .set_size_request((ww * 2.6) as i32, (wh * 2.2) as i32);
        }
    }

    /// Flicker state machine per window: steady shimmer, now
    /// and then a short flicker burst, rarely a real blackout —
    /// like a loose bulb rather than a disco.
    fn lamp_frame(&mut self) {
        let (w, h) = (self.w, self.h);
        for l in self.lamps.iter_mut() {
            let (wx, wy) = (l.fx * w as f64, l.fy * h as f64);
            let (ww, wh) = (l.fw * w as f64, l.fh * h as f64);
            self.layer.move_(&l.dark, wx, wy);
            // Glow patch centred on the window, wider spill.
            self.layer
                .move_(&l.glow, wx - ww * 0.8, wy - wh * 0.6);

            if l.out > 0 {
                // Blackout: dark patch on, glow off.
                l.out -= 1;
                l.dark.set_opacity(0.72);
                l.glow.set_opacity(0.0);
                continue;
            }
            l.dark.set_opacity(0.0);
            if l.burst > 0 {
                // Flicker burst: hard random brightness jumps.
                l.burst -= 1;
                let r = park_miller(&mut l.seed);
                l.glow
                    .set_opacity(if r < 0.15 { 0.05 } else { 0.10 + r * 0.35 });
                // ~1-in-40 bursts ends in a short blackout.
                if l.burst == 0 && park_miller(&mut l.seed) < 0.025 {
                    l.out = 4 + (park_miller(&mut l.seed) * 8.0) as u32;
                }
            } else {
                // Steady burn: gentle breathing around 0.28.
                let n = park_miller(&mut l.seed);
                l.glow.set_opacity(0.22 + n * 0.12);
                // Rare trigger: ~0.4 % per tick ≈ one burst / 16 s.
                if n > 0.996 {
                    l.burst = 6 + (park_miller(&mut l.seed) * 14.0) as u32;
                }
            }
        }
    }
}

/// Attach lamp overlays for the five light sources known in
/// the aurora-fjord artwork: three facade windows, the small
/// gable window and the lamp on the pier railing. Any other
/// image returns an empty set — no stray glow in the void.
fn build_lamps(image: &str, layer: &Fixed, seed: &mut u64) -> Vec<Lamp> {
    let name = std::path::Path::new(image)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name != "lieraos-default.png" && name != "aurora-fjord.png" {
        return Vec::new();
    }
    // (fx, fy, fw, fh) — window rects as fractions, measured on
    // the 3840×2161 artwork.
    const SPOTS: [(f64, f64, f64, f64); 5] = [
        (0.0755, 0.544, 0.0130, 0.034), // left facade, dim
        (0.1130, 0.539, 0.0185, 0.037), // centre facade, bright
        (0.1745, 0.539, 0.0195, 0.039), // gable wall, bright
        (0.1940, 0.500, 0.0105, 0.027), // attic window
        (0.2747, 0.592, 0.0105, 0.024), // pier railing lamp
    ];
    let mut lamps = Vec::new();
    for &(fx, fy, fw, fh) in SPOTS.iter() {
        let glow = Box::new(gtk4::Orientation::Horizontal, 0);
        glow.add_css_class("lamp-glow");
        glow.set_opacity(0.3);
        layer.put(&glow, 0.0, 0.0);
        let dark = Box::new(gtk4::Orientation::Horizontal, 0);
        dark.add_css_class("lamp-dark");
        dark.set_opacity(0.0);
        layer.put(&dark, 0.0, 0.0);
        lamps.push(Lamp {
            glow,
            dark,
            fx,
            fy,
            fw,
            fh,
            seed: *seed,
            burst: 0,
            out: 0,
        });
        *seed = (*seed * 48271) % 2147483647;
    }
    lamps
}

/// 1×1 transparent texture for the unloadable-image fallback.
fn empty_texture() -> gtk4::gdk::Texture {
    gtk4::gdk::MemoryTexture::new(
        1,
        1,
        gtk4::gdk::MemoryFormat::B8g8r8a8,
        &gtk4::glib::Bytes::from(&[0u8; 4]),
        4,
    )
    .upcast()
}

/// Park-Miller minimal standard — deterministic lamp timing.
fn park_miller(state: &mut u64) -> f64 {
    *state = (*state * 48271) % 2147483647;
    *state as f64 / 2147483647.0
}
