// Copyright (C) Jörg Schröder
// One layer-shell bar surface. Geometry: anchored to the configured
// screen edge, length steered via left/right margins relative to the
// monitor. Layout: three zone strips (start / centre / end) — the user
// assigns every applet to a zone in the properties dialog, so menu,
// tasklist, tray and clock can sit anywhere along the bar.
use crate::applets;
use crate::config::{Align, BarConfig};
use crate::toplevel::Store;
use crate::tray::Tray;
use crate::workspace::{WsInfo, WsStore};
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;

/// Shared per-bar state — live config plus the toplevel plumbing the
/// tasklist needs when the layout is rebuilt.
pub struct BarState {
    pub cfg: BarConfig,
    /// Per-bar CSS provider — restyled live when opacity/colour
    /// change, without touching other bars' styles.
    pub css: gtk4::CssProvider,
    /// Unique CSS class (`bar-N`) scoping this bar's style provider.
    pub css_class: String,
    pub store: Store,
    /// The tasklist's live zone cell — its single consumer was started
    /// once at spawn; layout rebuilds only re-register the widget.
    pub task_zone: Option<applets::tasklist::TaskZone>,
    /// Tray zone cell — its single consumer was started once at
    /// spawn; layout rebuilds only re-register the widget.
    pub tray: Tray,
    pub tray_zone: Option<applets::tray::TrayZone>,
    /// Workspace store + zone cell — same single-consumer pattern
    /// as the tasklist.
    pub ws_store: WsStore,
    pub ws_zone: Option<applets::workspace::WsZone>,
    /// Set by bars::remove before window.destroy() — every timer that
    /// still holds this bar must stop instead of resurrecting a
    /// destroyed surface via set_visible/apply.
    pub dead: bool,
    /// Gradient cycle position 0.0..1.0 for the animated background —
    /// lives here (not in the paint layer) so layout rebuilds keep
    /// the flow seamless instead of restarting the gradient.
    pub anim_phase: std::cell::Cell<f64>,
}

pub type State = Rc<RefCell<BarState>>;

/// Create, configure and present one bar window; returns the window
/// plus its shared state so the bars registry can manage it.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    app: &gtk4::Application,
    bar_id: usize,
    cfg: BarConfig,
    store: &Store,
    rx: Option<async_channel::Receiver<Vec<crate::toplevel::TopInfo>>>,
    tray: &Tray,
    tray_rx: Option<async_channel::Receiver<()>>,
    ws_store: &WsStore,
    ws_rx: Option<async_channel::Receiver<Vec<WsInfo>>>,
) -> (gtk4::Window, State) {
    let task_zone = rx.map(|rx| applets::tasklist::start(store, rx));
    let tray_zone = tray_rx.map(|rx| applets::tray::start(tray, rx, 24));
    let ws_zone = ws_rx.map(|rx| applets::workspace::start(ws_store, rx));
    let css = gtk4::CssProvider::new();
    // Display-wide provider scoped to this bar's unique class — the
    // modern replacement for per-widget style_context providers.
    if let Some(d) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &d,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let state: State = Rc::new(RefCell::new(BarState {
        cfg,
        css,
        css_class: format!("bar-{bar_id}"),
        store: store.clone(),
        task_zone,
        tray: tray.clone(),
        tray_zone,
        ws_store: ws_store.clone(),
        ws_zone,
        dead: false,
        anim_phase: std::cell::Cell::new(0.0),
    }));
    let window = gtk4::Window::builder().application(app).build();
    // The bar's visible background is painted by the root box —
    // the window must be transparent or its default backdrop would
    // stay opaque underneath and kill the alpha channel.
    window.add_css_class("bar-transparent");
    window.init_layer_shell();
    window.set_layer(Layer::Top);
    // Never accept keyboard input — game/app windows must always keep
    // focus; the bar is display+click only.
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_namespace(Some("ragnar-taskbar"));

    // Hide while any toplevel reports zwlr fullscreen — layer-shell
    // surfaces otherwise stay visible over borderless-maximised games,
    // which protocol-wise are not "fullscreen".
    {
        let (w, st) = (window.clone(), state.clone());
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(300), move || {
            // Bar was removed — die with it instead of resurrecting
            // the destroyed window via set_visible.
            if st.borrow().dead {
                return gtk4::glib::ControlFlow::Break;
            }
            // Hide only when a fullscreen toplevel covers THIS bar's
            // output — a fullscreen on the other monitor must not
            // drop the bar over there.
            let conn = w
                .surface()
                .and_then(|s| s.display().monitor_at_surface(&s))
                .and_then(|m| m.connector().map(|c| c.to_string()));
            let fs = st.borrow().store.tops.borrow().iter().any(|t| {
                t.fullscreen
                    && conn
                        .as_ref()
                        .map(|c| t.outputs.iter().any(|o| o == c))
                        .unwrap_or(false)
            });
            w.set_visible(!fs);
            gtk4::glib::ControlFlow::Continue
        });
    }

    // Right-click anywhere on the bar opens the bar menu — extensible:
    // new entries (transparency, applets, …) register here later.
    let click = gtk4::GestureClick::new();
    click.set_button(3);
    let (w, st) = (window.clone(), state.clone());
    click.connect_pressed(move |_, _, x, y| {
        // Only react on genuine bar background — a click that landed
        // on an applet (button, label, icon) belongs to that widget.
        if let Some(hit) = w.pick(x, y, gtk4::PickFlags::DEFAULT) {
            if hit.is::<gtk4::Button>() || hit.is::<gtk4::Label>() || hit.is::<gtk4::Image>() {
                return;
            }
        }
        open_menu(&w, &st, x, y);
    });
    window.add_controller(click);

    rebuild_root(&window, &state);

    // Margins need the monitor geometry — apply once mapped, then on
    // every later reconfiguration from the properties dialog.
    let st = state.clone();
    window.connect_map(move |w| apply(w, &st));
    // DisplayServices answers on D-Bus only once its service is up —
    // a second pass a few seconds later catches the startup race so
    // the primary pin lands even when the bar won it.
    {
        let (w, st) = (window.clone(), state.clone());
        gtk4::glib::timeout_add_seconds_local_once(5, move || {
            if !st.borrow().dead {
                apply(&w, &st);
            }
        });
    }
    window.present();
    (window, state)
}

/// Rebuild the three zone strips from the current layout — called at
/// spawn and whenever the user moves an applet to another zone.
pub fn rebuild_root(window: &gtk4::Window, state: &State) {
    let st = state.borrow();
    // Vertical bars stack their applets top-to-bottom.
    let vertical = st.cfg.edge == "left" || st.cfg.edge == "right";
    let root = gtk4::Box::new(
        if vertical {
            gtk4::Orientation::Vertical
        } else {
            gtk4::Orientation::Horizontal
        },
        6,
    );
    // The height request lives HERE, not in apply(): every layout
    // rebuild replaces the root box, and a fresh box would silently
    // drop back to its ~49px natural size — the slider looked dead
    // beyond that point.
    if vertical {
        root.set_size_request(st.cfg.height, -1);
    } else {
        root.set_size_request(-1, st.cfg.height);
    }
    root.add_css_class("ragnar-taskbar");
    root.add_css_class(&st.css_class);
    if vertical {
        root.add_css_class("vertical");
    }

    // The layout list renders strictly left-to-right: each item is an
    // applet widget or an expanding spacer that pushes neighbours
    // apart — several spacers create the visually separated groups.
    // Icons may never outgrow the bar itself — an oversize icon would
    // bleed into neighbour widgets instead of staying in its slot.
    let icon_px = (st.cfg.height - 12).clamp(8, st.cfg.icon_size);
    st.store.icon_size.set(icon_px);
    for item in &st.cfg.layout {
        let widget = match item.as_str() {
            _ if item == "spacer" || item.starts_with("spacer:") => {
                let sp = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
                if let Some(px) = item
                    .strip_prefix("spacer:")
                    .and_then(|n| n.parse::<i32>().ok())
                {
                    // Fixed gap the user sized in the preview — it
                    // applies along the bar's long axis.
                    if vertical {
                        sp.set_size_request(-1, px);
                    } else {
                        sp.set_size_request(px, -1);
                    }
                } else if vertical {
                    sp.set_vexpand(true);
                } else {
                    sp.set_hexpand(true);
                }
                sp.upcast()
            }
            "menu" => applets::menu::build(icon_px, &st.cfg.edge),
            "tasklist" => match &st.task_zone {
                Some(tz) => applets::tasklist::build(&st.store, tz, vertical),
                None => applets::tasklist::build_placeholder(vertical),
            },
            "workspace" => match &st.ws_zone {
                Some(wz) => {
                    applets::workspace::build(&st.ws_store, wz, vertical, st.cfg.workspace_max)
                }
                None => applets::workspace::build_placeholder(vertical),
            },
            "tray" => match &st.tray_zone {
                Some(tz) => applets::tray::build(&st.tray, tz, icon_px, vertical),
                None => gtk4::Box::new(gtk4::Orientation::Horizontal, 0).upcast(),
            },
            "sensor-cpu" => applets::sensor::build(state, applets::sensor::Kind::Cpu, vertical),
            "sensor-gpu" => applets::sensor::build(state, applets::sensor::Kind::Gpu, vertical),
            "sensor-ram" => applets::sensor::build(state, applets::sensor::Kind::Ram, vertical),
            "sensor-mb" => applets::sensor::build(state, applets::sensor::Kind::Mb, vertical),
            id if id.starts_with("cmd:") => applets::cmd::build(state, id),
            "status" => applets::status::build(icon_px, vertical),
            "notify" => applets::notify::build(state, icon_px, &st.cfg.edge),
            "clock" => applets::clock::build(state, vertical),
            _ => continue,
        };
        root.append(&widget);
    }
    // Animated background: the gradient layer sits UNDER the applets
    // in an overlay; the bar's own CSS background is transparent in
    // that mode (see apply), so the gradient shines through.
    if st.cfg.anim {
        let over = gtk4::Overlay::new();
        over.set_child(Some(&crate::bar_anim::paint_layer(state)));
        over.add_overlay(&root);
        window.set_child(Some(&over));
    } else {
        window.set_child(Some(&root));
    }
}

/// Small popover menu anchored at the pointer: the entry points for
/// everything bar-related. Kept deliberately thin — entries are added
/// as their features land.
fn open_menu(window: &gtk4::Window, _state: &State, x: f64, y: f64) {
    let pop = gtk4::Popover::new();
    pop.set_has_arrow(false);
    pop.set_parent(window);
    pop.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let props = gtk4::Button::with_label(&crate::i18n::tr("Eigenschaften …"));
    {
        let p = pop.clone();
        let w = window.clone();
        props.connect_clicked(move |_| {
            crate::settings::open(&w);
            p.popdown();
        });
    }
    props.set_has_frame(false);
    props.set_halign(gtk4::Align::Fill);
    list.append(&props);
    pop.set_child(Some(&list));
    pop.popup();
}

/// Push the current config's geometry/anchors to the compositor.
/// The primary output connector per Ragnar's DisplayServices —
/// `GetOutputs` returns (connector, name, vendor, serial, enabled,
/// mode, modes, pos, primary, scale, …); field 8 is the flag.
/// Async variant — a slow session bus at login must not stall the
/// whole bar while it waits on GetOutputs' reply.
async fn primary_connector() -> Option<String> {
    let conn = gtk4::gio::bus_get_future(gtk4::gio::BusType::Session)
        .await
        .ok()?;
    let v = conn
        .call_future(
            Some("org.lieraos.ragnar.DisplayServices"),
            "/org/lieraos/ragnar/DisplayServices",
            "org.lieraos.ragnar.DisplayServices",
            "GetOutputs",
            None,
            None,
            gtk4::gio::DBusCallFlags::NONE,
            2000,
        )
        .await
        .ok()?;
    let outs = v.child_value(0);
    for i in 0..outs.n_children() {
        let o = outs.child_value(i);
        if o.child_value(8).get::<bool>() == Some(true) {
            return o.child_value(0).str().map(|s| s.to_string());
        }
    }
    None
}

/// Resolve the target monitor asynchronously, then pin the window.
/// Runs off the call path so apply() stays non-blocking; with the
/// service up this lands in the same main-loop iteration, well before
/// the first frame.
fn pin_monitor(window: &gtk4::Window, configured: String, index: usize) {
    let w = window.clone();
    gtk4::glib::spawn_future_local(async move {
        // Explicit config wins; otherwise Ragnar's PRIMARY via
        // DisplayServices, falling back to the first monitor when the
        // service is silent.
        let want = if configured.is_empty() {
            primary_connector().await.unwrap_or_default()
        } else {
            configured
        };
        let Some(disp) = gtk4::gdk::Display::default() else {
            return;
        };
        let mons = disp.monitors();
        let pick = if want.is_empty() {
            mons.item(index as u32)
        } else {
            (0..mons.n_items()).find_map(|i| {
                mons.item(i).and_then(|m| {
                    m.downcast::<gtk4::gdk::Monitor>().ok().and_then(|mm| {
                        (mm.connector().map(|c| c.to_string()).as_deref() == Some(want.as_str()))
                            .then_some(mm.upcast::<gtk4::glib::Object>())
                    })
                })
            })
        };
        if let Some(m) = pick.and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok()) {
            w.set_monitor(Some(&m));
        }
    });
}

pub fn apply(window: &gtk4::Window, state: &State) {
    let cfg = state.borrow().cfg.clone();

    // Pin to an output — without this the compositor picks one
    // arbitrarily (the bar landed on the wrong monitor).
    pin_monitor(window, cfg.monitor.clone(), cfg.index);

    // Horizontal bars stretch across top/bottom edges; vertical bars
    // (left/right edge) stretch along the monitor's side instead.
    let horizontal = cfg.edge == "top" || cfg.edge == "bottom";
    window.set_anchor(Edge::Top, cfg.edge == "top" || !horizontal);
    window.set_anchor(Edge::Bottom, cfg.edge == "bottom" || !horizontal);
    window.set_anchor(Edge::Left, horizontal || cfg.edge == "left");
    window.set_anchor(Edge::Right, horizontal || cfg.edge == "right");
    window.set_exclusive_zone(cfg.height);
    if let Some(child) = window.child() {
        if horizontal {
            child.set_size_request(-1, cfg.height);
        } else {
            child.set_size_request(cfg.height, -1);
        }
    }

    // Free space along the LONG axis is distributed by alignment —
    // left/right margins for horizontal bars, top/bottom for vertical.
    let mon_long = window
        .surface()
        .and_then(|s| s.display().monitor_at_surface(&s))
        .map(|m| {
            let g = m.geometry();
            if horizontal {
                g.width()
            } else {
                g.height()
            }
        })
        .unwrap_or(0);
    let pct = cfg.width_percent.clamp(30, 100);
    let spare = (mon_long as i64 * (100 - pct) as i64 / 100) as i32;
    let (lead, trail) = match cfg.align {
        Align::Left => (0, spare),
        Align::Right => (spare, 0),
        Align::Center => (spare / 2, spare - spare / 2),
    };
    // All four margins must be set every time — switching orientation
    // leaves stale margins on the unused axis otherwise, which shows
    // as a huge gap between the bar and the screen edge.
    window.set_margin(Edge::Left, if horizontal { lead } else { 0 });
    window.set_margin(Edge::Right, if horizontal { trail } else { 0 });
    window.set_margin(Edge::Top, if horizontal { 0 } else { lead });
    window.set_margin(Edge::Bottom, if horizontal { 0 } else { trail });

    // Per-bar style: opacity composes on top of either the theme
    // background or the user's custom colour — GTK CSS `alpha()`
    // does the blending, so no colour parsing is needed.
    let alpha = (cfg.opacity.clamp(10, 100) as f64) / 100.0;
    // Animated bars paint their background in the gradient layer —
    // the root box must stay transparent or it would cover it.
    let bg = if cfg.anim {
        "transparent".to_string()
    } else if cfg.color.is_empty() {
        format!("alpha(@theme_bg_color, {alpha:.2})")
    } else if let Ok(c) = gtk4::gdk::RGBA::parse(&cfg.color) {
        let (r, g, b) = (c.red(), c.green(), c.blue());
        format!(
            "rgba({}, {}, {}, {alpha:.2})",
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8
        )
    } else {
        "@theme_bg_color".to_string()
    };
    let cls = state.borrow().css_class.clone();
    state
        .borrow()
        .css
        .load_from_data(&format!(".{cls} {{ background: {bg}; }}"));
}

/// Store an updated config and re-apply immediately (live preview).
pub fn update(window: &gtk4::Window, state: &State, cfg: BarConfig) {
    state.borrow_mut().cfg = cfg;
    apply(window, state);
}
