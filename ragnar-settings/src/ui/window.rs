// Copyright (C) Jörg Schröder
//! Main window: a global header bar (back button + search) above a
//! [`Stack`](gtk4::Stack) switching between the hub and an open panel.
//!
//! The back button lives in the persistent header, not inside the
//! panel page. Pages must not host the widget that destroys them:
//! when a widget is removed between press and release, GTK's implicit
//! pointer grab keeps pointing at the dead widget and the next click
//! is delivered into the void (the "dead click" after navigation).

use std::cell::RefCell;
use std::rc::Rc;

use libadwaita as adw;

use adw::prelude::*;
use adw::{Application, ApplicationWindow, HeaderBar, ToolbarView};
use gtk4::{Button, Orientation, ScrolledWindow, SearchEntry, Stack, StackTransitionType, Switch};

use crate::registry::{Action, Entry};

/// Stack page names.
const PAGE_HUB: &str = "hub";
const PAGE_PANEL: &str = "panel";

/// X11/Wayland mouse button number for the "back" side button
/// (XButton1). Listening to it separately keeps normal clicks
/// untouched.
const MOUSE_BACK_BUTTON: u32 = 8;

/// The application window and its navigation state.
pub struct SettingsWindow {
    window: ApplicationWindow,
    stack: Stack,
    back: Button,
    search: SearchEntry,
    hub_scroll: ScrolledWindow,
    /// Global advanced-mode switch living in the header bar.
    adv_switch: Switch,
    /// Last shown panel title, kept for potential header use.
    current: Rc<RefCell<String>>,
}

impl SettingsWindow {
    /// Create the window bound to `app`.
    pub fn new(app: &Application) -> Rc<Self> {
        let back = Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text(gettextrs::gettext("Back"))
            .visible(false)
            .build();
        let search = SearchEntry::builder()
            .placeholder_text(gettextrs::gettext("Search settings"))
            .width_chars(32)
            .build();
        let header = HeaderBar::new();
        header.pack_start(&back);
        header.set_title_widget(Some(&search));

        // Advanced mode switch in the header — visible on every
        // page, not just the hub, since panels rebuild on open
        // anyway and can never be stuck in a stale mode.
        let adv_switch = Switch::builder()
            .active(crate::advanced::enabled())
            .valign(gtk4::Align::Center)
            .build();
        let adv_box = gtk4::Box::new(Orientation::Horizontal, 8);
        adv_box.append(&gtk4::Label::new(Some(&gettextrs::gettext(
            "Advanced settings",
        ))));
        adv_box.append(&adv_switch);
        header.pack_end(&adv_box);

        let hub_scroll = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .build();

        // Footer on the hub page: configuration backup & restore.
        let footer = gtk4::Box::new(Orientation::Horizontal, 12);
        for m in ["top", "bottom", "start", "end"] {
            footer.set_property(&format!("margin-{m}"), 12);
        }
        let spacer = gtk4::Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        let backup_btn = Button::with_label(&gettextrs::gettext("Back up configuration…"));
        let restore_btn = Button::with_label(&gettextrs::gettext("Restore configuration…"));
        footer.append(&spacer);
        footer.append(&backup_btn);
        footer.append(&restore_btn);

        let hub_page = gtk4::Box::new(Orientation::Vertical, 0);
        hub_page.append(&hub_scroll);
        hub_page.append(&gtk4::Separator::new(Orientation::Horizontal));
        hub_page.append(&footer);

        let stack = Stack::builder()
            .transition_type(StackTransitionType::SlideLeftRight)
            .build();
        stack.add_named(&hub_page, Some(PAGE_HUB));
        stack.set_visible_child_name(PAGE_HUB);

        let toolbar = ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&stack));

        let window = ApplicationWindow::builder()
            .application(app)
            .content(&toolbar)
            .default_width(880)
            .default_height(600)
            .build();

        // Backup/restore buttons in the hub footer — both open a
        // file dialog transient for the window.
        {
            let w = window.clone().upcast::<gtk4::Window>();
            backup_btn.connect_clicked(move |_| crate::backup::export_dialog(&w));
        }
        {
            let w = window.clone().upcast::<gtk4::Window>();
            restore_btn.connect_clicked(move |_| crate::backup::restore_dialog(&w));
        }

        let this = Rc::new(Self {
            window,
            stack,
            back,
            search,
            hub_scroll,
            adv_switch,
            current: Rc::new(RefCell::new(String::new())),
        });
        this.wire_signals();
        this.show_hub("");
        this
    }

    /// Present the window.
    pub fn present(&self) {
        self.window.present();
    }

    /// Rebuild the hub contents honoring the current search text.
    fn show_hub(self: &Rc<Self>, query: &str) {
        let this = Rc::clone(self);
        let on_entry: Rc<dyn Fn(&'static Entry)> = Rc::new(move |entry| {
            this.open_entry(entry);
        });
        self.hub_scroll
            .set_child(Some(&super::hub::build(query, on_entry)));
    }

    /// Route the activated tile: show embedded panel or launch app.
    fn open_entry(&self, entry: &'static Entry) {
        match entry.action {
            Action::Panel(id) => {
                if let Some(widget) = crate::panels::build(id) {
                    let title = gettextrs::gettext(entry.title);
                    // Clear a stale grab before removing the old page:
                    // if the grab pointed into that page, unparenting
                    // it would strand the grab outside the window.
                    self.release_stale_pointer_grab();
                    self.replace_page(PAGE_PANEL, super::panel_host::wrap(widget));
                    self.stack.set_visible_child_name(PAGE_PANEL);
                    self.back.set_visible(true);
                    *self.current.borrow_mut() = title;
                }
            }
            Action::Launch(program) => {
                if let Err(err) = crate::launch::launch_program(program) {
                    tracing::warn!("launch failed: {err:#}");
                }
            }
        }
    }

    /// Open the embedded panel with registry id `id` (used by
    /// `--panel <id>`); unknown ids leave the hub showing.
    pub fn open_panel(&self, id: &str) {
        if let Some(entry) = crate::registry::all_entries()
            .iter()
            .find(|e| e.id == id && matches!(e.action, Action::Panel(_)))
        {
            self.open_entry(entry);
        }
    }

    /// Go back to the hub page.
    fn go_back(&self) {
        self.release_stale_pointer_grab();
        self.stack.set_visible_child_name(PAGE_HUB);
        self.back.set_visible(false);
    }

    /// Work around a GTK quirk: an implicit pointer grab created by a
    /// button press is only released for mouse buttons 1-5. Side
    /// buttons (e.g. "back", button 8) leave the grab behind, so it is
    /// reparented to the widget stack once the panel page hides — and
    /// the next click is then delivered to the stack instead of the
    /// tile under the pointer (the "dead click").
    ///
    /// Toggling `sensitive` on the toplevel triggers GTK's
    /// `update_pointer_focus_on_state_change`, which re-parents an
    /// implicit grab to the widget's parent. The window has no parent,
    /// so the grab is cleared outright and active-state accounting is
    /// reset. Both changes apply before the next frame, so nothing is
    /// ever rendered as insensitive.
    fn release_stale_pointer_grab(&self) {
        self.window.set_sensitive(false);
        self.window.set_sensitive(true);
    }

    /// Replace a named stack page, dropping the previous widget.
    fn replace_page(&self, name: &str, widget: impl IsA<gtk4::Widget>) {
        if let Some(old) = self.stack.child_by_name(name) {
            self.stack.remove(&old);
        }
        self.stack.add_named(&widget, Some(name));
    }

    /// Connect back button and search field.
    fn wire_signals(self: &Rc<Self>) {
        let this = Rc::clone(self);
        self.back.connect_clicked(move |_| this.go_back());

        let this = Rc::clone(self);
        self.search.connect_search_changed(move |entry| {
            // Searching jumps back to the hub and filters live. Do
            // NOT go through go_back(): its pointer-grab workaround
            // toggles `sensitive` on the toplevel, which drops
            // keyboard focus — every keystroke would kick the user
            // out of the search entry. Nothing is destroyed here, so
            // there is no grab that could go stale.
            this.stack.set_visible_child_name(PAGE_HUB);
            this.back.set_visible(false);
            this.show_hub(&entry.text());
        });

        // Mouse side buttons navigate back like in browsers and file
        // managers. Raw EventControllerLegacy is used instead of a
        // GestureClick: side-button presses create an implicit pointer
        // grab that reroutes the matching release to the grabbed
        // widget, so gesture handlers never see it. The legacy
        // controller observes every event at capture phase regardless
        // of grabs and child-claims. Any release of a button >5 also
        // clears a potentially stale grab (the "dead click" quirk).
        let mouse_side = gtk4::EventControllerLegacy::builder()
            .propagation_phase(gtk4::PropagationPhase::Capture)
            .build();
        let this = Rc::clone(self);
        mouse_side.connect_event(move |_, ev| {
            if ev.event_type() == gtk4::gdk::EventType::ButtonRelease {
                if let Some(be) = ev.downcast_ref::<gtk4::gdk::ButtonEvent>() {
                    let button = be.button();
                    if button == MOUSE_BACK_BUTTON && this.back.is_visible() {
                        this.go_back();
                    } else if button > 5 {
                        this.release_stale_pointer_grab();
                    }
                }
            }
            gtk4::glib::Propagation::Proceed
        });
        self.window.add_controller(mouse_side);

        // Advanced mode: persist, then rebuild the hub so tiles
        // appear/disappear immediately (keeping the search text).
        let this = Rc::clone(self);
        self.adv_switch.connect_state_notify(move |sw| {
            crate::advanced::set(sw.is_active());
            this.show_hub(&this.search.text());
        });
    }
}
