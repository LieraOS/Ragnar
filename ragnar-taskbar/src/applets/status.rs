// Status applet — the classic right-hand icons: network (wired /
// wireless / off), bluetooth and volume. State is read from real
// system sources on a 2 s timer: /sys/class/net and /sys/class/
// bluetooth for presence+power, wpctl for the PipeWire default sink.
// wpctl runs through gio::Subprocess — asynchronous: a stalled
// PipeWire must never freeze the whole bar's main loop.
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

/// First candidate the active icon theme actually provides — Mint-Y
/// and friends ship different name variants, so never trust a single
/// hardcoded name (a miss renders as an invisible empty image).
fn pick(cands: &[&str]) -> String {
    let theme = gtk4::IconTheme::for_display(&gtk4::gdk::Display::default().unwrap());
    cands
        .iter()
        .find(|n| theme.has_icon(n))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "image-missing".to_string())
}

pub fn build(size: i32, vertical: bool) -> gtk4::Widget {
    let zone = gtk4::Box::new(
        if vertical {
            gtk4::Orientation::Vertical
        } else {
            gtk4::Orientation::Horizontal
        },
        6,
    );
    // Network and Bluetooth need no local icons — Heimdall and
    // Blueman publish their own StatusNotifierItems through the
    // real tray. Only the volume state is displayed locally.
    let vol = gtk4::Image::new();
    vol.set_pixel_size(size);
    // The icon lives inside a flat button so it can receive clicks —
    // left: per-app faders, right: menu, scroll: master volume.
    let btn = gtk4::Button::new();
    btn.set_child(Some(&vol));
    btn.set_has_frame(false);
    zone.append(&btn);
    let prev: std::rc::Rc<std::cell::RefCell<Option<(f64, bool)>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    refresh(vol.clone(), prev.clone());
    glib::timeout_add_seconds_local(2, {
        let prev = prev.clone();
        move || {
            refresh(vol.clone(), prev.clone());
            glib::ControlFlow::Continue
        }
    });

    // Left click → per-application fader popover.
    let streams = super::volume_popup::stream_popover(&btn);
    btn.connect_clicked(move |_| streams.popup());

    // Right click → open Skald / toggle effects menu.
    let menu = super::volume_popup::menu_popover(&btn);
    let right = gtk4::GestureClick::new();
    right.set_button(3);
    right.connect_pressed(move |_, _, _, _| menu.popup());
    btn.add_controller(right);

    // Scroll on the icon steps the session default volume — the next
    // 2 s poll picks the new value up for the icon text.
    let scroll = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    {
        let prev = prev.clone();
        scroll.connect_scroll(move |_, _, dy| {
            super::volume_popup::scroll_step(&prev, dy);
            glib::Propagation::Stop
        });
    }
    btn.add_controller(scroll);
    zone.upcast()
}

/// Apply one wpctl answer to the icon — parsing lives apart from the
/// async fetch so it stays unit-testable.
fn apply_volume(
    vol: &gtk4::Image,
    out: Option<String>,
    prev: &std::cell::RefCell<Option<(f64, bool)>>,
) {
    let (icon, tip) = match out {
        Some(s) => {
            let muted = s.contains("MUTED");
            let level = s
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            // Fire the configured event sound on a real change —
            // never on the first poll (nothing to compare yet).
            {
                let mut p = prev.borrow_mut();
                if let Some((old_level, old_muted)) = *p {
                    if old_level != level || old_muted != muted {
                        let _ = std::process::Command::new("ragnar-skald-sound")
                            .arg("audio-volume-change")
                            .spawn();
                    }
                }
                *p = Some((level, muted));
            }
            // Prefer monochrome symbolic icons — the hicolor fallback
            // theme ships them under an xsi- prefix.
            let name = if muted {
                "audio-volume-muted-symbolic"
            } else if level > 0.66 {
                "audio-volume-high-symbolic"
            } else if level > 0.33 {
                "audio-volume-medium-symbolic"
            } else {
                "audio-volume-low-symbolic"
            };
            (
                pick(&[name, &format!("xsi-{name}")]),
                if muted {
                    crate::i18n::tr("Stumm")
                } else {
                    crate::i18n::trf("Lautstärke {}%", &[&format!("{:.0}", level * 100.0)])
                },
            )
        }
        None => (
            pick(&[
                "audio-volume-muted-symbolic",
                "xsi-audio-volume-muted-symbolic",
            ]),
            crate::i18n::tr("Kein Audio"),
        ),
    };
    vol.set_icon_name(Some(&icon));
    vol.set_tooltip_text(Some(&tip));
}

/// Kick one async wpctl poll; the icon updates when the answer lands.
/// The main loop never waits on the child process.
fn refresh(vol: gtk4::Image, prev: std::rc::Rc<std::cell::RefCell<Option<(f64, bool)>>>) {
    glib::spawn_future_local(async move {
        let text = match gio::Subprocess::newv(
            &[
                std::ffi::OsStr::new("wpctl"),
                std::ffi::OsStr::new("get-volume"),
                std::ffi::OsStr::new("@DEFAULT_AUDIO_SINK@"),
            ],
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
        ) {
            // The process object stays alive across the await — the
            // communicate future borrows it until completion.
            Ok(p) => p
                .communicate_utf8_future(None)
                .await
                .ok()
                .and_then(|(stdout, _)| stdout)
                .map(|g| g.to_string()),
            Err(_) => None,
        };
        apply_volume(&vol, text, &prev);
    });
}
