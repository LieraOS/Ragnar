// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — GL preview for the livewall effects editor
//
// A small GtkGLArea running the very fragment shader that
// ragnar-livewall installs under /usr/share/ragnar-livewall/ —
// the dialog preview therefore renders exactly what the desktop
// will show. GL plumbing lives in fx_preview_gl.rs.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{GLArea, glib};

use super::fx_preview_gl as gl_part;

/// Re-export for wallpaper.rs — the live check whether the
/// preview can run (shader files installed).
pub use super::fx_preview_gl::shader_available;

/// Editable effect + edit-highlight state shared with the
/// dialog — the render pass reads it every frame.
/// `edit` carries the shader's u_edit id: 0 none, 1 glow,
/// 2 stars, 3 mist, 4 snow, 5 rain, 6 fireflies, 7 clouds.
#[derive(Clone)]
pub struct FxView {
    pub aurora_rect: [f32; 4],
    /// intensity, enabled
    pub aurora: [f32; 2],
    pub stars_rects: [[f32; 4]; 4],
    /// density, speed, region count (0 = off)
    pub stars: [f32; 3],
    pub mist_rects: [[f32; 4]; 4],
    /// opacity, drift, region count (0 = off)
    pub mist: [f32; 3],
    pub snow_rects: [[f32; 4]; 4],
    /// density, fall speed, region count
    pub snow: [f32; 3],
    pub rain_rects: [[f32; 4]; 4],
    /// strength, speed, region count
    pub rain: [f32; 3],
    pub fly_rects: [[f32; 4]; 4],
    /// density, drift speed, region count
    pub fly: [f32; 3],
    pub cloud_rects: [[f32; 4]; 4],
    /// opacity, speed, region count
    pub cloud: [f32; 3],
    pub edit: f32,
}

/// Effect ids, matching the shader's u_edit values.
pub const FX_GLOW: u8 = 1;
pub const FX_STARS: u8 = 2;
pub const FX_MIST: u8 = 3;
pub const FX_SNOW: u8 = 4;
pub const FX_RAIN: u8 = 5;
pub const FX_FLIES: u8 = 6;
pub const FX_CLOUDS: u8 = 7;

impl FxView {
    /// (rects, params) of a multi-region effect — None for the
    /// single-band glow.
    pub fn region_fx(
        &mut self,
        id: u8,
    ) -> Option<(&mut [[f32; 4]; 4], &mut [f32; 3])> {
        match id {
            FX_STARS => Some((&mut self.stars_rects, &mut self.stars)),
            FX_MIST => Some((&mut self.mist_rects, &mut self.mist)),
            FX_SNOW => Some((&mut self.snow_rects, &mut self.snow)),
            FX_RAIN => Some((&mut self.rain_rects, &mut self.rain)),
            FX_FLIES => Some((&mut self.fly_rects, &mut self.fly)),
            FX_CLOUDS => Some((&mut self.cloud_rects, &mut self.cloud)),
            _ => None,
        }
    }

    /// Read-only rect list for counters.
    pub fn region_rects(&self, id: u8) -> Option<&[[f32; 4]; 4]> {
        match id {
            FX_STARS => Some(&self.stars_rects),
            FX_MIST => Some(&self.mist_rects),
            FX_SNOW => Some(&self.snow_rects),
            FX_RAIN => Some(&self.rain_rects),
            FX_FLIES => Some(&self.fly_rects),
            FX_CLOUDS => Some(&self.cloud_rects),
            _ => None,
        }
    }

    /// Param slot: glow idx 0 = intensity (idx 1 = enabled);
    /// region effects idx 0/1 = params, idx 2 = enabled count.
    pub fn param_mut(&mut self, id: u8, idx: usize) -> &mut f32 {
        match id {
            FX_GLOW => &mut self.aurora[idx],
            FX_STARS => &mut self.stars[idx],
            FX_MIST => &mut self.mist[idx],
            FX_SNOW => &mut self.snow[idx],
            FX_RAIN => &mut self.rain[idx],
            FX_FLIES => &mut self.fly[idx],
            _ => &mut self.cloud[idx],
        }
    }
}

impl Default for FxView {
    /// Matches ragnar-livewall's defaults: glow on in the
    /// classic full-image band, everything else off.
    fn default() -> Self {
        Self {
            aurora_rect: [0.0, 0.0, 1.0, 1.0],
            aurora: [1.0, 1.0],
            stars_rects: [[0.0; 4]; 4],
            stars: [0.7, 1.0, 0.0],
            mist_rects: [[0.0; 4]; 4],
            mist: [0.35, 1.0, 0.0],
            snow_rects: [[0.0; 4]; 4],
            snow: [0.7, 1.0, 0.0],
            rain_rects: [[0.0; 4]; 4],
            rain: [0.6, 1.0, 0.0],
            fly_rects: [[0.0; 4]; 4],
            fly: [0.6, 1.0, 0.0],
            cloud_rects: [[0.0; 4]; 4],
            cloud: [0.4, 1.0, 0.0],
            edit: 0.0,
        }
    }
}

/// The preview widget — drives its own ~20 fps clock so the
/// dialog always shows the live animation.
pub struct FxPreview {
    pub area: GLArea,
    pub fx: Rc<RefCell<FxView>>,
    state: Rc<gl_part::Gl>,
    start: std::time::Instant,
}

impl FxPreview {
    /// GL preview for `image` — falls back to an empty area
    /// (hidden) when GL or the shader file is unavailable.
    pub fn new(texture: &gtk4::gdk::Texture) -> Self {
        let state = Rc::new(gl_part::Gl::default());
        let mut dl = gtk4::gdk::TextureDownloader::new(texture);
        dl.set_format(gtk4::gdk::MemoryFormat::R8g8b8a8);
        let (bytes, stride) = dl.download_bytes();
        let _ = stride;
        *state.pixels.borrow_mut() = Some(bytes.to_vec());
        state.img.set((texture.width(), texture.height()));

        let fx = Rc::new(RefCell::new(FxView::default()));
        let area = GLArea::new();
        area.set_auto_render(false);
        area.set_size_request(480, 300);

        let st = state.clone();
        area.connect_realize(move |area| {
            if gl_part::init_gl(area, &st).is_err() {
                st.failed.set(true);
                area.set_visible(false);
            }
        });
        let st = state.clone();
        let fxv = fx.clone();
        let start = std::time::Instant::now();
        area.connect_render(move |_, _| {
            if !st.failed.get() {
                gl_part::draw(&st, &fxv.borrow(), start.elapsed().as_secs_f32());
            }
            glib::Propagation::Stop
        });
        Self {
            area,
            fx,
            state,
            start,
        }
    }

    /// Kick one frame — called from the dialog's tick.
    pub fn tick(&self) {
        let _ = self.start;
        if !self.state.failed.get() {
            self.area.queue_render();
        }
    }
}

