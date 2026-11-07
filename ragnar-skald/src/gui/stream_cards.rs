//! Per-app stream cards — one volume card per app playing audio.
//!
//! `Stream/Output/Audio` nodes appear when an app starts playing and
//! vanish when it stops, like the Windows volume mixer. Several
//! streams of one app (two Firefox videos, media + notification)
//! merge into a SINGLE card that drives them all — the user thinks
//! in apps, not in streams.
//!
//! Card updates are debounced ~400 ms: one-shot notification sounds
//! pop a stream for a blink only and must not flash a card.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{glib, Box, Frame, Label, Orientation, Scale, ToggleButton, Widget};

use crate::pipewire::backend::PwBackend;
use crate::pipewire::types::{AppStream, BackendEvent};

/// How long a stream list change settles before the cards update.
const APPEAR_DELAY: Duration = Duration::from_millis(400);

/// Widgets of one app card. `ids` are the app's live stream ids —
/// every fader/mute action applies to all of them; `updating` is
/// shared between sync path and signal closures to suppress
/// feedback loops.
struct StreamCard {
    frame: Frame,
    scale: Scale,
    mute: ToggleButton,
    /// Shared live id list — the signal closures read this at action
    /// time so fader/mute always hit the app's CURRENT streams, even
    /// when Firefox opens or closes a video in between.
    ids: Rc<RefCell<Vec<u32>>>,
    updating: Rc<Cell<bool>>,
}

/// The dynamic "Anwendungen" section on the output page.
pub struct StreamCards {
    container: Box,
    section: Frame,
    list: Box,
    cards: RefCell<HashMap<String, StreamCard>>,
    pending: RefCell<Vec<AppStream>>,
    pending_source: RefCell<Option<glib::SourceId>>,
    backend: Rc<PwBackend>,
}

impl StreamCards {
    /// Build the (initially hidden) section.
    pub fn new(backend: Rc<PwBackend>) -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .build();
        let list = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .build();
        let section = Frame::builder()
            .label("Anwendungen")
            .label_xalign(0.0)
            .child(&list)
            .visible(false)
            .build();
        container.append(&section);
        Self {
            container,
            section,
            list,
            cards: RefCell::new(HashMap::new()),
            pending: RefCell::new(Vec::new()),
            pending_source: RefCell::new(None),
            backend,
        }
    }

    /// Top-level widget for the output panel.
    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Route backend events: list changes are debounced, single
    /// updates sync straight through.
    pub fn apply_event(self: &Rc<Self>, event: &BackendEvent) {
        match event {
            BackendEvent::StreamsChanged(list) => self.schedule_rebuild(list.clone()),
            BackendEvent::StreamChanged(stream) => self.sync(stream),
            _ => {}
        }
    }

    /// Store the latest stream list and (re)arm the appear delay —
    /// flickering streams collapse into the final list.
    fn schedule_rebuild(self: &Rc<Self>, list: Vec<AppStream>) {
        *self.pending.borrow_mut() = list;
        if self.pending_source.borrow().is_some() {
            return; // timer already armed with freshest list
        }
        let me = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(APPEAR_DELAY, move || {
            if let Some(me) = me.upgrade() {
                *me.pending_source.borrow_mut() = None;
                let list = std::mem::take(&mut *me.pending.borrow_mut());
                me.rebuild(&list);
            }
        });
        *self.pending_source.borrow_mut() = Some(id);
    }

    /// Rebuild the card set grouped by app name: drop vanished apps,
    /// add newly appearing ones, sync faders of continuing apps.
    fn rebuild(&self, streams: &[AppStream]) {
        // Group stream ids and representative volume/mute per app.
        let mut apps: HashMap<String, (Vec<u32>, f32, bool)> = HashMap::new();
        for s in streams {
            let e = apps
                .entry(s.app_name.clone())
                .or_insert_with(|| (Vec::new(), s.volume, s.muted));
            e.0.push(s.id);
            e.1 = e.1.max(s.volume);
            e.2 &= s.muted;
        }
        self.cards.borrow_mut().retain(|app, card| {
            let keep = apps.contains_key(app);
            if !keep {
                self.list.remove(&card.frame);
            }
            keep
        });
        for (app, (ids, vol, mute)) in &apps {
            if !self.cards.borrow().contains_key(app) {
                let card = self.build_card(app, ids.clone());
                self.list.append(&card.frame);
                self.cards.borrow_mut().insert(app.clone(), card);
            }
            let mut cards = self.cards.borrow_mut();
            if let Some(card) = cards.get_mut(app) {
                *card.ids.borrow_mut() = ids.clone();
                card.updating.set(true);
                card.scale.set_value(f64::from(*vol) * 100.0);
                card.mute.set_active(*mute);
                card.updating.set(false);
            }
        }
        self.section.set_visible(!apps.is_empty());
    }

    /// Sync a single stream change: find its app's card and show the
    /// loudest stream's volume as the app volume.
    fn sync(&self, stream: &AppStream) {
        if let Some(card) = self.cards.borrow().get(&stream.app_name) {
            if card.ids.borrow().contains(&stream.id) {
                card.updating.set(true);
                card.scale.set_value(f64::from(stream.volume) * 100.0);
                card.mute.set_active(stream.muted);
                card.updating.set(false);
            }
        }
    }

    /// One card: app label + fader + mute toggle inside a Frame.
    fn build_card(&self, app: &str, ids: Vec<u32>) -> StreamCard {
        let row = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .margin_start(10)
            .margin_end(10)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        row.append(
            &Label::builder()
                .label(app)
                .xalign(0.0)
                .hexpand(false)
                .width_chars(18)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build(),
        );
        // Fader capped at 100 %: stream × device gains multiply
        // (1.5 × 1.5 = 225 % → hard clipping), so only the device
        // slider carries the boost — an app may go to full volume,
        // never beyond it.
        let scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .hexpand(true)
            .build();
        scale.set_range(0.0, 100.0);
        scale.set_increments(1.0, 10.0);
        scale.set_value_pos(gtk4::PositionType::Right);
        row.append(&scale);
        let mute = ToggleButton::builder().label("M").build();
        row.append(&mute);
        let frame = Frame::builder().child(&row).build();
        let updating = Rc::new(Cell::new(false));
        // Every action fans out to all of the app's CURRENT streams.
        let ids_shared = Rc::new(RefCell::new(ids));
        let ids_move = Rc::clone(&ids_shared);
        let backend = Rc::clone(&self.backend);
        let flag = Rc::clone(&updating);
        scale.connect_value_changed(move |s| {
            if flag.get() {
                return;
            }
            let v = (s.value() / 100.0) as f32;
            for id in ids_move.borrow().iter() {
                backend.set_volume(*id, v);
            }
        });
        let ids_move = Rc::clone(&ids_shared);
        let backend = Rc::clone(&self.backend);
        let flag = Rc::clone(&updating);
        mute.connect_toggled(move |b| {
            if flag.get() {
                return;
            }
            for id in ids_move.borrow().iter() {
                backend.set_mute(*id, b.is_active());
            }
        });
        StreamCard {
            frame,
            scale,
            mute,
            ids: ids_shared,
            updating,
        }
    }
}
