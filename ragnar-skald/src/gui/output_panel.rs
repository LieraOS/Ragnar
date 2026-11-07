//! Output panel — sink endpoints tab plus built-in effects.
//!
//! Stacks the shared [`EndpointPanel`] (device list, volume, mute,
//! boost) above the [`EffectsWidget`] (native filter-chain effects).

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box, Orientation, Separator, Widget};

use crate::config::{AppSettings, ConfigPersistence};
use crate::gui::effects_widget::EffectsWidget;
use crate::gui::endpoint_panel::EndpointPanel;
use crate::gui::stream_cards::StreamCards;
use crate::pipewire::backend::PwBackend;
use crate::pipewire::types::{BackendEvent, Direction};

/// Output device settings panel (sinks) including the effects section.
pub struct OutputPanel {
    container: Box,
    inner: EndpointPanel,
    streams: Rc<StreamCards>,
    effects: Rc<EffectsWidget>,
}

impl OutputPanel {
    /// Build the panel; `settings`/`persist` carry the saved effect
    /// state that the widget restores from and writes back to.
    pub fn new(
        backend: Rc<PwBackend>,
        settings: Rc<RefCell<AppSettings>>,
        persist: Rc<ConfigPersistence>,
    ) -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .build();
        let inner = EndpointPanel::new(Direction::Output, Rc::clone(&backend));
        let streams = Rc::new(StreamCards::new(Rc::clone(&backend)));
        let effects = Rc::new(EffectsWidget::new(backend, settings, persist));
        // Picking a real output device while the chain is loaded must
        // tear the chain down — otherwise streams route around it in
        // parallel and the summed signal clips hard.
        {
            let effects = Rc::clone(&effects);
            inner.set_on_raw_select(move || effects.disable());
        }
        container.append(inner.widget());
        container.append(streams.widget());
        container.append(&Separator::new(Orientation::Horizontal));
        container.append(effects.widget());
        Self {
            container,
            inner,
            streams,
            effects,
        }
    }

    /// Re-apply the persisted effect state (call once the backend is
    /// connected so the chain loads when it was enabled previously).
    pub fn restore_effects(&self) {
        self.effects.restore();
    }

    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Route a backend event into all sections.
    pub fn apply_event(&self, event: &BackendEvent) {
        self.inner.apply_event(event);
        self.streams.apply_event(event);
        self.effects.apply_event(event);
    }
}
