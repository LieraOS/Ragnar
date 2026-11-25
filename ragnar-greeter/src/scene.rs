// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-greeter — animated backdrop
//
// The LIERA artwork plus the same ambient effects the Plymouth
// bootsplash plays: a soft glow drifting slowly across the
// ornament band and occasional lightning strikes in the sky.
//
// Widget structure: a GtkOverlay whose main child is the
// background Picture — the overlay always fills the window, so
// the artwork cover-scales to whatever size we get. Everything
// positioned by pixel (glow, bolts, flash, login panel) lives in
// a GtkFixed added as an overlay child; overlay children do NOT
// contribute to the natural size, which keeps sprite
// size_requests from feeding back into the window size (the
// zoom loop the previous all-Fixed layout had).
// A 50 ms tick moves the sprites; geometry is re-checked every
// frame so mode/resize changes recentre automatically.

use gtk4::prelude::*;
use gtk4::{glib, Box, Fixed, Overlay, Picture};

use crate::util::{park_miller, picture};

/// Frame cadence — the same 50 ms the Plymouth refresh used.
const TICK_MS: u32 = 50;

/// All animated state for the backdrop.
pub struct Scene {
    /// Full-window overlay: bg as main child, Fixed layer on top.
    root: Overlay,
    /// Pixel-positioned layer (natural size ignored by overlay).
    layer: Fixed,
    /// Gold radial-gradient widget (CSS class `glow-beam`) — a
    /// tinted widget, not the white PNG: white on the bright
    /// nebula artwork was invisible at any opacity.
    glow: Box,
    bolts: [Picture; 2],
    /// White gradient widget for the strike burst (`strike-flash`).
    flash: Box,
    /// Login panel inside `layer` — relayout() places it
    /// centre-x, ~62 % down, like the GRUB menu under the ornament.
    panel: gtk4::Widget,
    /// Primary-monitor rect inside window coords — the panel
    /// centres on this, not the full multi-monitor span.
    anchor: (f64, f64, f64, f64),
    // geometry cached for the resize check
    w: i32,
    h: i32,
    // animation counters (see lieraos.script for the model)
    tick: u64,
    strike_t: u32,
    strike_silent: bool,
    strike_bolt: usize,
    next_strike: u64,
    /// Follow-up bolt time for double strikes (u64::MAX = none).
    pending_strike: u64,
    /// Twinkling star specks in the sky band.
    stars: Vec<Star>,
    // glow run state — the beam rests (dim breathing) and
    // periodically sweeps a random path/speed/height
    glow_active: bool,
    glow_t: u32,
    glow_len: u32,
    glow_from: (f64, f64),
    glow_to: (f64, f64),
    next_glow: u64,
    rnd: u64,
}

/// A pulsing point of light — fractional position in the sky
/// band plus its own twinkle phase/speed.
struct Star {
    widget: Box,
    fx: f64,
    fy: f64,
    phase: f64,
    speed: f64,
}

impl Scene {
    /// Build the layer stack. `panel` is the login box — placed
    /// by relayout() at the artwork-relative position.
    pub fn new(panel: &impl IsA<gtk4::Widget>) -> std::rc::Rc<std::cell::RefCell<Self>> {
        let bg = picture("background.png");
        bg.set_content_fit(gtk4::ContentFit::Cover);

        // Gradient widgets styled via the app CSS — gold for the
        // drifting beam, white for the lightning burst.
        let glow = Box::new(gtk4::Orientation::Horizontal, 0);
        glow.add_css_class("glow-beam");
        glow.set_opacity(0.0);

        let bolts = [picture("bolt_a.png"), picture("bolt_b.png")];
        let flash = Box::new(gtk4::Orientation::Horizontal, 0);
        flash.add_css_class("strike-flash");
        for b in &bolts {
            b.set_opacity(0.0);
        }
        flash.set_opacity(0.0);

        // Pixel layer — carries every sprite and the panel. Its
        // natural size is ignored by the overlay, so requests
        // can never grow the window.
        let layer = Fixed::new();
        layer.put(&bolts[0], 0.0, 0.0);
        layer.put(&bolts[1], 0.0, 0.0);
        layer.put(&flash, 0.0, 0.0);
        layer.put(&glow, 0.0, 0.0);
        let panel_widget = panel.clone().upcast::<gtk4::Widget>();
        layer.put(&panel_widget, 0.0, 0.0);

        let root = Overlay::new();
        root.set_child(Some(&bg));
        root.add_overlay(&layer);

        // Star specks — positions/phases drawn from the same
        // deterministic stream so every boot looks identical-ish.
        let mut seed = 987654321u64;
        let mut stars = Vec::new();
        for _ in 0..14 {
            let w_ = Box::new(gtk4::Orientation::Horizontal, 0);
            w_.add_css_class("twinkle");
            let size = 4 + (park_miller(&mut seed) * 8.0) as i32;
            w_.set_size_request(size, size);
            w_.set_opacity(0.0);
            layer.put(&w_, 0.0, 0.0);
            stars.push(Star {
                widget: w_,
                fx: park_miller(&mut seed),
                fy: park_miller(&mut seed) * 0.45,
                phase: park_miller(&mut seed) * std::f64::consts::TAU,
                speed: 0.10 + park_miller(&mut seed) * 0.35,
            });
        }

        let scene = std::rc::Rc::new(std::cell::RefCell::new(Self {
            root,
            layer,
            glow,
            bolts,
            flash,
            panel: panel_widget,
            anchor: (0.0, 0.0, 0.0, 0.0),
            w: 0,
            h: 0,
            tick: 0,
            strike_t: 0,
            strike_silent: false,
            strike_bolt: 0,
            next_strike: 60,
            pending_strike: u64::MAX,
            stars,
            glow_active: false,
            glow_t: 0,
            glow_len: 1,
            glow_from: (0.0, 0.4),
            glow_to: (0.0, 0.4),
            next_glow: 40,
            rnd: seed,
        }));

        let s = scene.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(TICK_MS as u64), move || {
            s.borrow_mut().frame();
            glib::ControlFlow::Continue
        });

        scene
    }

    /// The widget to set as the window child.
    pub fn widget(&self) -> Overlay {
        self.root.clone()
    }

    /// Tell the scene where the primary monitor sits inside the
    /// window (0,0,0,0 = single monitor, use the full layer).
    /// Panel + sprites span all screens, the login box stays
    /// centred on the primary one.
    pub fn set_anchor(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.anchor = (x, y, w, h);
        self.relayout();
    }

    /// One animation frame: geometry check, glow sweep, lightning.
    fn frame(&mut self) {
        self.tick += 1;

        // Recentre sprites when the allocation changed — same
        // strategy the Plymouth script uses for its mode switch.
        let (w, h) = (self.root.width(), self.root.height());
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.relayout();
        }

        self.glow_frame();
        self.strike_frame();
        self.stars_frame();
    }

    /// Position every sprite for the current allocation. Only
    /// sizes inside the overlay layer — never the window.
    fn relayout(&mut self) {
        let (w, h) = (self.w, self.h);
        if w <= 0 || h <= 0 {
            return;
        }
        // Box widgets have no natural size — request both axes.
        self.glow
            .set_size_request(self.glow_width(), (h as f64 * 0.35) as i32);
        let bolt_h = (h as f64 * 0.30) as i32;
        for b in &self.bolts {
            b.set_size_request(-1, bolt_h);
        }
        self.flash.set_size_request(560, 280);

        // Stars keep fractional positions in the sky band.
        for s in &self.stars {
            self.layer
                .move_(&s.widget, s.fx * w as f64, s.fy * h as f64);
        }

        // Panel centre-x at 62 % of the height — under the
        // ornament band (~45 %), where the GRUB menu sits.
        // On multi-monitor the anchor rect scopes this to the
        // primary screen so the box doesn't land between panels.
        let (_, pw, _, _) = self.panel.measure(gtk4::Orientation::Horizontal, -1);
        let (_, ph, _, _) = self.panel.measure(gtk4::Orientation::Vertical, -1);
        let (ax, ay, aw, ah) = self.anchor;
        let (aw, ah) = if aw > 0.0 { (aw, ah) } else { (w as f64, h as f64) };
        self.layer.move_(
            &self.panel,
            ax + (aw - pw as f64).max(0.0) / 2.0,
            ay + ah * 0.62 - ph as f64 / 2.0,
        );
    }

    /// Glow width scaled to screen (source strip ~480 px wide).
    fn glow_width(&self) -> i32 {
        (self.w as f64 * 0.35).max(220.0) as i32
    }

    /// Beam behaviour: between runs it rests dim (breathing);
    /// every few seconds it sweeps a random path — direction,
    /// speed and height vary (straight or diagonal), fading in
    /// and out along the way.
    fn glow_frame(&mut self) {
        let (w, h) = (self.w, self.h);
        if w <= 0 {
            return;
        }

        // Start a run when the pause expires.
        if !self.glow_active && self.tick >= self.next_glow {
            self.glow_active = true;
            self.glow_t = 0;
            self.glow_len = 100 + (self.next_rnd() * 180.0) as u32;
            let y = 0.28 + self.next_rnd() * 0.28;
            self.glow_from = match (self.next_rnd() * 4.0) as u32 {
                // right→left
                1 => (1.05, y),
                // diagonal top-left → bottom-right
                2 => (-0.30, 0.20),
                // diagonal top-right → bottom-left
                3 => (1.05, 0.20),
                // left→right (most common shape)
                _ => (-0.35, y),
            };
            self.glow_to = match (self.next_rnd() * 4.0) as u32 {
                1 => (-0.35, y),
                2 => (1.05, 0.58),
                3 => (-0.30, 0.58),
                _ => (1.05, y),
            };
        }

        if self.glow_active {
            // Smoothstep easing — soft start/stop, fast middle.
            self.glow_t += 1;
            let p = (self.glow_t as f64 / self.glow_len as f64).min(1.0);
            let e = p * p * (3.0 - 2.0 * p);
            let gw = self.glow_width() as f64;
            let x = (self.glow_from.0 + (self.glow_to.0 - self.glow_from.0) * e)
                * w as f64
                - gw / 2.0;
            let y = (self.glow_from.1 + (self.glow_to.1 - self.glow_from.1) * e)
                * h as f64;
            self.layer.move_(&self.glow, x, y);
            // Fade in, bright middle, fade out.
            self.glow
                .set_opacity(0.20 + 0.50 * (p * std::f64::consts::PI).sin());
            if p >= 1.0 {
                self.glow_active = false;
                self.next_glow = self.tick + 100 + (self.next_rnd() * 160.0) as u64;
            }
        } else {
            // Resting: faint breathing presence at the last spot.
            let breathe = (self.tick % 100) as f64;
            let breathe = if breathe > 50.0 { 100.0 - breathe } else { breathe };
            self.glow.set_opacity(0.10 + 0.12 * breathe / 50.0);
        }
    }

    /// Deterministic Park-Miller step — only strike timing/position.
    fn next_rnd(&mut self) -> f64 {
        park_miller(&mut self.rnd)
    }

    /// Strike begins when the timer expires, then plays the
    /// bright-dip-bright-fade sequence (~0.5 s at 50 ms ticks).
    fn strike_frame(&mut self) {
        let (w, h) = (self.w, self.h);
        if w <= 0 {
            return;
        }
        if self.strike_t == 0 && self.tick >= self.next_strike {
            self.strike_t = 1;
            self.strike_bolt = (self.next_rnd() * 2.0) as usize % 2;
            // Three sky zones: left, right, or — rarer — centre.
            let side = self.next_rnd();
            let bolt_x = if side < 0.40 {
                w as f64 * (0.05 + self.next_rnd() * 0.23)
            } else if side < 0.80 {
                w as f64 * (0.72 + self.next_rnd() * 0.23)
            } else {
                w as f64 * (0.32 + self.next_rnd() * 0.36)
            };
            let bolt_y = h as f64 * (0.02 + self.next_rnd() * 0.05);
            self.layer.move_(&self.bolts[self.strike_bolt], bolt_x, bolt_y);
            self.layer.move_(&self.flash, bolt_x - 220.0, bolt_y - 60.0);
            // Silent = flash-only "cloud lighting" — rare, real
            // bolt shapes are the point of the show.
            self.strike_silent = self.next_rnd() < 0.10;
            // ~30 % chance of a quick follow-up strike elsewhere.
            self.pending_strike = if self.next_rnd() < 0.30 {
                self.tick + 6 + (self.next_rnd() * 14.0) as u64
            } else {
                u64::MAX
            };
        }
        if self.strike_t > 0 {
            self.strike_t += 1;
            let o = match self.strike_t {
                t if t <= 5 => 1.0,
                t if t <= 8 => 0.12,
                t if t <= 12 => 0.85,
                t => (0.85 - (t - 12) as f64 * 0.08).max(0.0),
            };
            self.bolts[self.strike_bolt]
                .set_opacity(o * if self.strike_silent { 0.0 } else { 1.0 });
            self.flash.set_opacity(o * 0.4);
            if o <= 0.0 {
                self.strike_t = 0;
                // Follow-up bolt wins over the random interval.
                self.next_strike = if self.pending_strike != u64::MAX {
                    self.pending_strike
                } else {
                    // 3–7 s between strikes — requested cadence.
                    self.tick + 60 + (self.next_rnd() * 80.0) as u64
                };
                self.pending_strike = u64::MAX;
            }
        }
    }

    /// Twinkle — each star pulses at its own phase and speed;
    /// squaring the sine makes peaks bright and troughs faint.
    fn stars_frame(&mut self) {
        for s in &self.stars {
            let v = ((self.tick as f64 * s.speed + s.phase).sin() * 0.5 + 0.5).powi(2);
            s.widget.set_opacity(0.10 + 0.55 * v);
        }
    }
}
