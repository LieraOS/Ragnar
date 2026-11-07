//! Firewall page: a beginner-friendly UFW front-end. One on/off
//! switch, two plain-language default policies and a list of open
//! ports that can be added or removed by mouse — everything runs
//! through the privileged `ragnar-heimdall-helper` via pkexec.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use libadwaita as adw;

use adw::prelude::*;
use adw::{ActionRow, ComboRow, PreferencesGroup, PreferencesPage, Spinner, SwitchRow};
use gtk4::{Align, Button, Image, StringList};

use crate::backend::{self, UfwRule, UfwStatus};

/// Poll interval for worker results arriving on the main loop.
const POLL_MS: u64 = 200;

/// Page state shared across async refresh/mutation callbacks.
struct Fw {
    page: PreferencesPage,
    groups: RefCell<Vec<PreferencesGroup>>,
    status: RefCell<Option<UfwStatus>>,
    busy: Cell<bool>,
    notice: RefCell<Option<String>>,
}

/// Rc-Wrapper, damit das FwOps-Trait den freien `mutate`-Pfad nutzt.
struct FwRc(Rc<Fw>);

impl super::firewall_adv::FwOps for FwRc {
    fn mutate(&self, op: Box<dyn FnOnce() -> anyhow::Result<()> + Send + 'static>) {
        mutate(self.0.clone(), op);
    }
}

/// Build the firewall page. When UFW is not installed the page is
/// reduced to an honest status group instead of dead controls.
pub fn build(stack: &gtk4::Stack) -> gtk4::Widget {
    let page = PreferencesPage::new();
    let fw = Rc::new(Fw {
        page: page.clone(),
        groups: RefCell::new(Vec::new()),
        status: RefCell::new(None),
        busy: Cell::new(false),
        notice: RefCell::new(None),
    });

    if !backend::ufw_available() {
        let group = PreferencesGroup::builder()
            .title(gettextrs::gettext("Firewall"))
            .build();
        let row = ActionRow::builder()
            .title(gettextrs::gettext("UFW is not installed"))
            .subtitle(gettextrs::gettext(
                "Install the ufw package to manage the firewall.",
            ))
            .build();
        row.add_prefix(&Image::from_icon_name("dialog-warning-symbolic"));
        group.add(&row);
        page.add(&group);
        return page.upcast();
    }

    // Defer the privileged `ufw status` read until the Firewall page is
    // actually selected — otherwise opening the app prompts for admin
    // authentication just to render an off-screen tab. GtkStack keeps
    // every child mapped for transitions, so `connect_map` is not a
    // usable signal here; watch `visible-child-name` instead.
    let loaded = Rc::new(Cell::new(false));
    let on_shown: Rc<dyn Fn()> = {
        let fw = Rc::clone(&fw);
        let loaded = Rc::clone(&loaded);
        Rc::new(move || {
            if !loaded.replace(true) {
                refresh(Rc::clone(&fw));
            }
        })
    };
    stack.connect_notify_local(Some("visible-child-name"), {
        let on_shown = Rc::clone(&on_shown);
        move |stack, _| {
            if stack.visible_child_name().as_deref() == Some(super::sidebar::View::Firewall.tag()) {
                on_shown();
            }
        }
    });
    if stack.visible_child_name().as_deref() == Some(super::sidebar::View::Firewall.tag()) {
        on_shown();
    }
    page.upcast()
}

/// (Re)load `ufw status` in a worker thread, then re-render.
fn refresh(fw: Rc<Fw>) {
    fw.busy.set(true);
    render(&fw);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(backend::ufw_status());
    });
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(result) => {
                fw.busy.set(false);
                match result {
                    Ok(status) => {
                        *fw.status.borrow_mut() = Some(status);
                        *fw.notice.borrow_mut() = None;
                    }
                    Err(err) => {
                        tracing::warn!("ufw status failed: {err:#}");
                        *fw.notice.borrow_mut() = Some(format!("{err:#}"));
                    }
                }
                render(&fw);
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Run a mutating ufw call in a worker, then refresh the status.
fn mutate<F: FnOnce() -> anyhow::Result<()> + Send + 'static>(fw: Rc<Fw>, op: F) {
    fw.busy.set(true);
    render(&fw);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = op();
        let _ = tx.send(result);
    });
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(result) => {
                fw.busy.set(false);
                if let Err(err) = result {
                    tracing::warn!("ufw operation failed: {err:#}");
                    *fw.notice.borrow_mut() = Some(format!("{err:#}"));
                }
                refresh(fw.clone());
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Rebuild all groups from the current state.
fn render(fw: &Rc<Fw>) {
    for group in fw.groups.borrow_mut().drain(..) {
        fw.page.remove(&group);
    }

    if let Some(msg) = fw.notice.borrow().clone() {
        let group = PreferencesGroup::new();
        let row = ActionRow::builder().subtitle(msg).build();
        row.add_prefix(&Image::from_icon_name("dialog-warning-symbolic"));
        group.add(&row);
        add_group(fw, &group);
    }

    if fw.busy.get() {
        let group = PreferencesGroup::new();
        let row = ActionRow::builder()
            .title(gettextrs::gettext("Working…"))
            .build();
        row.add_suffix(&Spinner::new());
        group.add(&row);
        add_group(fw, &group);
        return;
    }

    let Some(status) = fw.status.borrow().clone() else {
        let group = PreferencesGroup::new();
        group.add(
            &ActionRow::builder()
                .title(gettextrs::gettext("Firewall status could not be read"))
                .build(),
        );
        add_group(fw, &group);
        return;
    };

    add_group(fw, &status_group(fw, &status));
    add_group(fw, &defaults_group(fw, &status));
    add_group(fw, &rules_group(fw, &status));
    add_group(
        fw,
        &super::firewall_adv::group(Rc::new(RefCell::new(FwRc(fw.clone()))), &status),
    );
}

/// Track a freshly added group for the next render pass.
fn add_group(fw: &Rc<Fw>, group: &PreferencesGroup) {
    fw.page.add(group);
    fw.groups.borrow_mut().push(group.clone());
}

/// Group 1: the master switch plus a plain-language status line.
fn status_group(fw: &Rc<Fw>, status: &UfwStatus) -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Firewall"))
        .build();

    let row = SwitchRow::builder()
        .title(gettextrs::gettext("Firewall protection"))
        .subtitle(gettextrs::gettext("Blocks unwanted incoming connections."))
        .active(status.enabled)
        .build();
    let fw2 = Rc::clone(fw);
    row.connect_active_notify(move |sw| {
        let on = sw.is_active();
        mutate(Rc::clone(&fw2), move || backend::ufw_set_enabled(on));
    });
    group.add(&row);
    group
}

/// Group 2: default policies in beginner wording.
fn defaults_group(fw: &Rc<Fw>, status: &UfwStatus) -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Default behavior"))
        .description(gettextrs::gettext(
            "Recommended for desktop PCs: block incoming, allow outgoing. Established connections stay unaffected.",
        ))
        .build();

    group.add(&policy_combo(
        fw,
        gettextrs::gettext("Incoming connections"),
        vec![gettextrs::gettext("Block"), gettextrs::gettext("Allow")],
        status.incoming == backend::UfwPolicy::Allow,
        "incoming",
    ));
    group.add(&policy_combo(
        fw,
        gettextrs::gettext("Outgoing connections"),
        vec![gettextrs::gettext("Allow"), gettextrs::gettext("Block")],
        status.outgoing != backend::UfwPolicy::Allow,
        "outgoing",
    ));
    group
}

/// One ComboRow mapping a friendly label pair onto a ufw default.
fn policy_combo(
    fw: &Rc<Fw>,
    title: String,
    labels: Vec<String>,
    deny_selected: bool,
    direction: &'static str,
) -> ComboRow {
    let model = StringList::new(&[]);
    for label in &labels {
        model.append(label);
    }
    let combo = ComboRow::builder()
        .title(&title)
        .model(&model)
        .selected(u32::from(deny_selected))
        .build();
    let fw = Rc::clone(fw);
    combo.connect_selected_notify(move |combo| {
        // For "incoming" index 0 means deny; for "outgoing" it is
        // index 1 — the labels above encode that order.
        let deny = if direction == "incoming" {
            combo.selected() == 0
        } else {
            combo.selected() == 1
        };
        mutate(Rc::clone(&fw), move || {
            backend::ufw_set_default(direction, !deny)
        });
    });
    combo
}

/// Group 3: the rule list plus the add action on top.
fn rules_group(fw: &Rc<Fw>, status: &UfwStatus) -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Ports and rules"))
        .description(gettextrs::gettext(
            "Decide which ports programs may listen on.",
        ))
        .build();

    let add = ActionRow::builder()
        .title(gettextrs::gettext("Open or close a port…"))
        .activatable(true)
        .build();
    add.add_prefix(&Image::from_icon_name("list-add-symbolic"));
    {
        let fw2 = Rc::clone(fw);
        add.connect_activated(move |_| {
            let fw3 = Rc::clone(&fw2);
            let fw4 = Rc::clone(&fw3);
            super::firewall_add::present(&fw3.page, move |allow, port, proto, from| {
                mutate(Rc::clone(&fw4), move || {
                    backend::ufw_add_rule(allow, &port, proto, &from)
                });
            });
        });
    }
    group.add(&add);

    if status.rules.is_empty() {
        group.add(
            &ActionRow::builder()
                .title(gettextrs::gettext("No ports configured"))
                .subtitle(gettextrs::gettext(
                    "The default policy applies to all traffic.",
                ))
                .build(),
        );
    }
    for rule in &status.rules {
        group.add(&rule_row(fw, rule));
    }
    group
}

/// One firewall rule: port/protocol, action and a delete button.
fn rule_row(fw: &Rc<Fw>, rule: &UfwRule) -> ActionRow {
    let allowed = rule.action.contains("ALLOW");
    let direction = if rule.action.contains("OUT") {
        gettextrs::gettext("outgoing")
    } else {
        gettextrs::gettext("incoming")
    };
    let verb = if allowed {
        gettextrs::gettext("Open")
    } else {
        gettextrs::gettext("Blocked")
    };
    let mut subtitle = format!(
        "{verb} · {direction} · {} {}",
        gettextrs::gettext("from"),
        rule.from
    );
    if rule.ipv6 {
        subtitle.push_str(" (IPv6)");
    }

    let row = ActionRow::builder()
        .title(&rule.to)
        .subtitle(subtitle)
        .build();
    row.add_prefix(&Image::from_icon_name(if allowed {
        "emblem-ok-symbolic"
    } else {
        "action-unavailable-symbolic"
    }));

    let trash = Button::builder()
        .icon_name("user-trash-symbolic")
        .valign(Align::Center)
        .tooltip_text(gettextrs::gettext("Remove rule"))
        .build();
    trash.add_css_class("flat");
    {
        let fw = Rc::clone(fw);
        let number = rule.number;
        trash.connect_clicked(move |_| {
            mutate(Rc::clone(&fw), move || backend::ufw_delete_rule(number));
        });
    }
    row.add_suffix(&trash);
    row
}
