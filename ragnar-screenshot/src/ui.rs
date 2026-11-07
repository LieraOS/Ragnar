// SPDX-License-Identifier: GPL-3.0-or-later
// Main window: options page (mode, cursor, sound, delay, take
// button) that swaps to a result page after each capture.

use crate::i18n::tr;
use crate::model::{CapturedImage, Mode, Options};
use crate::{overlay, wayland::Wayland};
use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct AppUi {
    pub window: adw::ApplicationWindow,
    stack: gtk4::Stack,
    mode_tiles: gtk4::Box,
    monitor_combo: gtk4::DropDown,
    cursor_sw: gtk4::Switch,
    sound_sw: gtk4::Switch,
    delay_spin: gtk4::SpinButton,
    preview: gtk4::Picture,
    edit_slot: gtk4::Box,
    wl: Rc<RefCell<Wayland>>,
    shot: Rc<RefCell<Option<CapturedImage>>>,
}

impl AppUi {
    pub fn new(app: &adw::Application) -> Rc<Self> {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(tr("Screenshot"))
            .default_width(420)
            .default_height(360)
            .build();
        let wl = Rc::new(RefCell::new(
            Wayland::new().unwrap_or_else(|e| panic!("wayland: {e}")),
        ));
        // Transparent background for the layer-shell overlay windows.
        let css = gtk4::CssProvider::new();
        css.load_from_data(
            "window.ragnar-shot-overlay { background: transparent; box-shadow: none; }",
        );
        if let Some(d) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &d,
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        // --- Options page ------------------------------------------------
        let opts = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        margins(&opts, 18);

        // Mode tiles: All / Monitor / Window / Area — clickable
        // cards like the GNOME picker, active one highlighted.
        // GNOME-style segmented selector: one linked box with flat
        // toggle segments, big icon on top, label underneath — the
        // active segment gets the pressed/selected look for free.
        let tiles = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        tiles.set_homogeneous(true);
        tiles.add_css_class("linked");
        let tile_defs = [
            ("video-display-symbolic", tr("All monitors")),
            ("computer-symbolic", tr("Monitor")),
            ("window-maximize-symbolic", tr("Window")),
            ("edit-select-all-symbolic", tr("Area")),
        ];
        let mut first: Option<gtk4::ToggleButton> = None;
        for (icon, label) in tile_defs {
            let tile = gtk4::ToggleButton::new();
            tile.add_css_class("flat");
            let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
            inner.set_margin_top(10);
            inner.set_margin_bottom(10);
            let img = gtk4::Image::from_icon_name(icon);
            img.set_pixel_size(32);
            inner.append(&img);
            let l = gtk4::Label::new(Some(&label));
            l.set_wrap(true);
            inner.append(&l);
            tile.set_child(Some(&inner));
            if let Some(ref f) = first {
                tile.set_group(Some(f));
            } else {
                tile.set_active(true);
                first = Some(tile.clone());
            }
            tiles.append(&tile);
        }
        opts.append(&labeled(&tr("Mode"), tiles.clone()));
        // Per-monitor picker — only visible when the Monitor tile
        // is active.
        let mnames: Vec<String> = {
            let w = wl.borrow();
            // Dedupe only identical (name, position) pairs — labwc
            // may advertise the same name on distinct outputs.
            let mut seen = std::collections::HashSet::new();
            w.monitors()
                .iter()
                .filter(|m| seen.insert((m.name.clone(), m.x, m.y)))
                .map(|m| format!("{} ({}x{})", m.name, m.w, m.h))
                .collect()
        };
        let mrefs: Vec<&str> = mnames.iter().map(|s| s.as_str()).collect();
        let mlist = gtk4::StringList::new(&mrefs);
        let monitor_combo = gtk4::DropDown::new(Some(mlist), gtk4::Expression::NONE);
        let monitor_row = labeled(&tr("Monitor"), monitor_combo.clone());
        monitor_row.set_visible(false);
        opts.append(&monitor_row);
        // Hint shown only in Window mode — the click on the target
        // window is what activates it for the capture.
        let win_hint = gtk4::Label::new(Some(&tr("Click the window you want to capture")));
        win_hint.add_css_class("dim-label");
        win_hint.set_visible(false);
        {
            let hint = win_hint.clone();
            let row = monitor_row.clone();
            let t = tiles.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                let mut it = t.first_child();
                let mut i = 0usize;
                let mut active = 0usize;
                while let Some(w) = it {
                    if w.downcast_ref::<gtk4::ToggleButton>()
                        .is_some_and(|b| b.is_active())
                    {
                        active = i;
                    }
                    it = w.next_sibling();
                    i += 1;
                }
                row.set_visible(active == 1);
                hint.set_visible(active == 2);
                glib::ControlFlow::Continue
            });
        }

        let cursor_sw = gtk4::Switch::new();
        opts.append(&labeled(&tr("Show pointer"), cursor_sw.clone()));
        let sound_sw = gtk4::Switch::new();
        sound_sw.set_active(true);
        opts.append(&labeled(&tr("Play sound"), sound_sw.clone()));
        let delay_spin = gtk4::SpinButton::with_range(0.0, 30.0, 1.0);
        delay_spin.set_digits(0);
        opts.append(&labeled(&tr("Delay (s)"), delay_spin.clone()));

        opts.append(&win_hint);

        let take = gtk4::Button::with_label(&tr("Take screenshot"));
        take.add_css_class("suggested-action");
        take.set_margin_top(12);
        opts.append(&take);

        // --- Result page ---------------------------------------------------
        let res = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
        margins(&res, 12);
        let preview = gtk4::Picture::new();
        preview.set_hexpand(true);
        preview.set_vexpand(true);
        preview.set_can_shrink(true);
        res.append(&preview);
        let btnbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        btnbox.set_halign(gtk4::Align::Center);
        for (label, css) in [
            ("Save", "suggested-action"),
            ("Edit", ""),
            ("Retake", ""),
            ("Close", ""),
        ] {
            let b = gtk4::Button::with_label(&tr(label));
            if !css.is_empty() {
                b.add_css_class(css);
            }
            btnbox.append(&b);
        }
        res.append(&btnbox);

        let stack = gtk4::Stack::new();
        stack.add_named(&opts, Some("options"));
        stack.add_named(&res, Some("result"));
        // Editor page gets rebuilt per shot — this is just its slot.
        let edit_slot = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        stack.add_named(&edit_slot, Some("edit"));
        // Header bar gives drag handle, title and close button.
        let bar = adw::HeaderBar::new();
        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        root.append(&bar);
        root.append(&stack);
        window.set_content(Some(&root));

        let ui = Rc::new(Self {
            window: window.clone(),
            stack,
            mode_tiles: tiles,
            monitor_combo,
            cursor_sw,
            sound_sw,
            delay_spin,
            preview,
            edit_slot,
            wl,
            shot: Rc::new(RefCell::new(None)),
        });

        take.connect_clicked(glib::clone!(
            #[weak]
            ui,
            move |_| ui.start_capture()
        ));
        // Wire the four result buttons by walking the box children.
        let mut it = btnbox.first_child();
        for name in ["save", "edit", "retake", "close"] {
            if let Some(w) = it {
                it = w.next_sibling();
                let b = w.downcast::<gtk4::Button>().unwrap();
                match name {
                    "save" => {
                        b.connect_clicked(glib::clone!(
                            #[weak]
                            ui,
                            move |_| ui.save()
                        ));
                    }
                    "edit" => {
                        b.connect_clicked(glib::clone!(
                            #[weak]
                            ui,
                            move |_| ui.open_editor()
                        ));
                    }
                    "retake" => {
                        b.connect_clicked(glib::clone!(
                            #[weak]
                            ui,
                            move |_| ui.back()
                        ));
                    }
                    _ => {
                        b.connect_clicked(glib::clone!(
                            #[weak]
                            ui,
                            move |_| ui.close_all()
                        ));
                    }
                }
            }
        }
        ui
    }

    fn options(&self) -> Options {
        // Which tile is active → mode. Monitor defaults to output 0
        // (a monitor picker can follow once multiple outputs matter).
        let mut n = 0usize;
        let mut it = self.mode_tiles.first_child();
        let mut i = 0usize;
        while let Some(w) = it {
            if w.downcast_ref::<gtk4::ToggleButton>()
                .is_some_and(|b| b.is_active())
            {
                n = i;
            }
            it = w.next_sibling();
            i += 1;
        }
        let mode = match n {
            0 => Mode::All,
            1 => Mode::Monitor(self.monitor_combo.selected() as usize),
            2 => Mode::Window,
            _ => Mode::Area,
        };
        Options {
            mode,
            show_cursor: self.cursor_sw.is_active(),
            play_sound: self.sound_sw.is_active(),
            delay_secs: self.delay_spin.value_as_int() as u32,
        }
    }

    /// Hide the window, wait the chosen delay, then run the chosen
    /// capture path.
    pub fn start_capture(self: &Rc<Self>) {
        let o = self.options();
        self.window.set_visible(false);
        let ui = self.clone();
        glib::timeout_add_local_once(
            std::time::Duration::from_millis(300 + o.delay_secs as u64 * 1000),
            move || match o.mode {
                Mode::All => ui.finish(WaylandGuard::all(&ui.wl, o.show_cursor)),
                Mode::Monitor(i) => {
                    ui.finish(WaylandGuard::monitor(&ui.wl, i, None, o.show_cursor))
                }
                Mode::Area => ui.pick_area(o),
                Mode::Window => ui.pick_window(),
            },
        );
    }

    /// Selection path: fullscreen overlays, then capture the picked
    /// monitor region.
    fn pick_area(self: &Rc<Self>, o: Options) {
        let ui = self.clone();
        // Logical rects + monitor list for the overlay.
        let (rects, gmonitors): (Vec<_>, Vec<_>) = {
            let wl = self.wl.borrow();
            let r = wl.monitors().iter().map(|m| (m.x, m.y, m.w, m.h)).collect();
            let d = gdk::Display::default().unwrap();
            let gm = d.monitors();
            let mut v = Vec::new();
            for i in 0..gm.n_items() {
                if let Some(m) = gm.item(i).and_downcast::<gdk::Monitor>() {
                    v.push(m);
                }
            }
            (r, v)
        };
        overlay::run(&rects, &gmonitors, move |sel| {
            match sel {
                // Overlays close on selection — wait a beat so the
                // compositor drops them, otherwise the selection
                // border would end up inside the captured pixels.
                Some((idx, x, y, w, h)) => {
                    let u2 = ui.clone();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(200),
                        move || {
                            u2.finish(WaylandGuard::monitor(
                                &u2.wl,
                                idx,
                                Some((x, y, w, h)),
                                o.show_cursor,
                            ))
                        },
                    );
                }
                None => ui.back(), // Escape → back to options.
            }
        });
    }

    /// Window mode: the dialog hides, the user clicks whichever
    /// window they want — that click activates the toplevel
    /// (zwlr_foreign_toplevel state), which we then shoot through
    /// ext_image_copy_capture. Times out back to the dialog.
    fn pick_window(self: &Rc<Self>) {
        let r = crate::toplevel::WindowPick::new().and_then(|mut p| p.capture_activated(10_000));
        match r {
            Ok(img) => self.finish(Ok(img)),
            Err(e) => {
                eprintln!("[ragnar-screenshot] window capture: {e}");
                self.back();
            }
        }
    }

    /// Got a frame — show preview + action buttons, play sound.
    fn finish(self: &Rc<Self>, r: Result<CapturedImage, String>) {
        match r {
            Ok(img) => {
                if self.options().play_sound {
                    shutter();
                }
                let tex = texture(&img);
                self.preview.set_paintable(Some(&tex));
                *self.shot.borrow_mut() = Some(img);
                self.stack.set_visible_child_name("result");
                self.window.set_visible(true);
            }
            Err(e) => {
                eprintln!("[ragnar-screenshot] capture failed: {e}");
                self.back();
            }
        }
    }

    /// Open the editor page for the captured image.
    fn open_editor(self: &Rc<Self>) {
        let img = self.shot.borrow().as_ref().map(|_| ());
        if img.is_none() {
            return;
        }
        let shot = self.shot.borrow();
        let Some(img) = shot.as_ref() else { return };
        let ui = self.clone();
        let editor = crate::editor::Editor::new(img, move |res| {
            // Baked image → replace the shot and refresh preview.
            if let Some(img) = res {
                let tex = texture(&img);
                ui.preview.set_paintable(Some(&tex));
                *ui.shot.borrow_mut() = Some(img);
            }
            ui.stack.set_visible_child_name("result");
        });
        // Rebuild the slot for this shot.
        while let Some(c) = self.edit_slot.first_child() {
            self.edit_slot.remove(&c);
        }
        self.edit_slot.append(&editor.widget);
        self.stack.set_visible_child_name("edit");
        editor.widget.grab_focus();
    }

    /// Back to the options page (Escape, retake, failure).
    fn back(self: &Rc<Self>) {
        self.clear_clip();
        self.stack.set_visible_child_name("options");
        self.window.set_visible(true);
    }

    fn save(self: &Rc<Self>) {
        if self.shot.borrow().is_none() {
            return;
        }
        let dialog = gtk4::FileDialog::new();
        dialog.set_initial_name(Some("screenshot.png"));
        let ui = self.clone();
        let win = self.window.clone();
        dialog.save(Some(&win), gio::Cancellable::NONE, move |res| {
            if let Ok(f) = res {
                if let Some(img) = ui.shot.borrow().as_ref() {
                    let tex = texture(img);
                    if let Some(path) = f.path() {
                        if let Err(e) = tex.save_to_png(&path) {
                            eprintln!("[ragnar-screenshot] save failed: {e}");
                        }
                    }
                }
            }
            ui.clear_clip();
        });
    }

    /// Clipboard hygiene: nothing of the screenshot may linger.
    fn clear_clip(&self) {
        if let Some(d) = gdk::Display::default() {
            let _ = d.clipboard().set_content(None::<&gdk::ContentProvider>);
        }
    }

    fn close_all(self: &Rc<Self>) {
        self.clear_clip();
        self.window.close();
    }
}

/// Set the same margin on all four sides (no set_margin_all in gtk4-rs 0.9).
fn margins(w: &impl IsA<gtk4::Widget>, n: i32) {
    w.set_margin_top(n);
    w.set_margin_bottom(n);
    w.set_margin_start(n);
    w.set_margin_end(n);
}

/// Small helper so the borrow of Wayland stays inside one call.
struct WaylandGuard;
impl WaylandGuard {
    fn all(wl: &Rc<RefCell<Wayland>>, cursor: bool) -> Result<CapturedImage, String> {
        wl.borrow_mut().capture_all(cursor)
    }
    fn monitor(
        wl: &Rc<RefCell<Wayland>>,
        idx: usize,
        rect: Option<(i32, i32, i32, i32)>,
        cursor: bool,
    ) -> Result<CapturedImage, String> {
        wl.borrow_mut().capture_monitor(idx, rect, cursor)
    }
}

/// CapturedImage → MemoryTexture for preview/save/clipboard.
fn texture(img: &CapturedImage) -> gdk::Texture {
    let bytes = glib::Bytes::from_owned(img.pixels.clone());
    gdk::MemoryTexture::new(
        img.width as i32,
        img.height as i32,
        gdk::MemoryFormat::B8g8r8a8,
        &bytes,
        img.width * 4,
    )
    .upcast()
}

/// Label + widget row used for every option.
fn labeled(text: &str, w: impl IsA<gtk4::Widget>) -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let l = gtk4::Label::new(Some(text));
    l.set_hexpand(true);
    l.set_xalign(0.0);
    b.append(&l);
    b.append(&w);
    b
}

/// Camera-shutter sound — freedesktop event sound via pw-play,
/// silent fallback if the file or the player is missing.
fn shutter() {
    let f = "/usr/share/sounds/freedesktop/stereo/camera-shutter.oga";
    for player in ["pw-play", "paplay", "canberra-gtk-play"] {
        if let Ok(c) = std::process::Command::new(player).arg(f).spawn() {
            std::thread::spawn(move || {
                let _ = c;
            });
            return;
        }
    }
}
