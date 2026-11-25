// Copyright (C) Jörg Schröder
//! Input panel — source endpoints tab plus microphone effects.
//!
//! Stacks the shared [`EndpointPanel`] (device list, volume, mute,
//! boost) above the [`MicEffectsWidget`] (preset, noise gate, echo,
//! level meter, monitor test). Selecting an input endpoint aims the
//! level meter at it.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box, Orientation, Separator, Widget};

use crate::config::{AppSettings, ConfigPersistence};
use crate::gui::endpoint_panel::EndpointPanel;
use crate::gui::mic_effects::MicEffectsWidget;
use crate::pipewire::backend::PwBackend;
use crate::pipewire::types::{BackendEvent, Direction};

/// Input device settings panel (sources/microphones).
pub struct InputPanel {
    container: Box,
    inner: EndpointPanel,
    effects: Rc<MicEffectsWidget>,
}

impl InputPanel {
    /// Build the panel; `settings`/`persist` carry the saved mic-effect
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
        let inner = EndpointPanel::new(Direction::Input, Rc::clone(&backend));
        let effects = Rc::new(MicEffectsWidget::new(
            Rc::clone(&backend),
            settings,
            persist,
        ));

        // The meter follows the mic the user selected in the list.
        inner.set_on_select(move |id| backend.set_meter_target(Some(id)));

        container.append(inner.widget());
        container.append(&Separator::new(Orientation::Horizontal));
        container.append(effects.widget());
        Self {
            container,
            inner,
            effects,
        }
    }

    /// Re-apply the persisted mic-effect state (call once the backend
    /// is connected so the daemon is spawned when needed).
    pub fn restore_effects(&self) {
        self.effects.restore();
    }

    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Route a backend event into both sections.
    pub fn apply_event(&self, event: &BackendEvent) {
        self.inner.apply_event(event);
        self.effects.apply_event(event);
    }
}
