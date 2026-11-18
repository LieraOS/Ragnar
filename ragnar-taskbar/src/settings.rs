// Taskbar settings window — one place to manage every bar on the
// desktop. Left column: a card per bar (selectable, deletable) plus
// a prominent add card. Right column: the selected bar's settings —
// visual monitor edge picker, geometry sliders and applet content.
// Everything commits live; there is no OK/Cancel.
use gtk4::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod anim_section;
mod bar_chip;
mod bar_page;
mod launcher_dlg;
mod menu_page;
mod workspace_ctl;

/// Shared handles both panes need — the window itself, the card list,
/// the detail pane and the index of the bar currently being edited.
pub struct Ui {
    pub list: gtk4::Box,
    pub detail: gtk4::Box,
    pub selected: Rc<Cell<usize>>,
    /// true = the detail pane shows the start-menu page, not a bar.
    pub menu_page: Rc<Cell<bool>>,
    /// Category currently picked on the menu page — survives rebuilds.
    pub menu_sel: Rc<RefCell<String>>,
    /// The settings window itself — transient parent for dialogs.
    pub win: gtk4::Window,
    /// Draggable divider between the bar column and the detail pane —
    /// the user sizes the column freely instead of a fixed width.
    pub paned: gtk4::Paned,
    /// Sidebar swapper: "full" = bar cards, "back" = slim back strip
    /// shown while the menu manager owns the window's width.
    pub side_stack: gtk4::Stack,
    /// Paned position remembered when the menu page collapses the
    /// sidebar — restored on the way back to the bars.
    pub saved_pos: Rc<Cell<i32>>,
}

/// Open (or focus) the settings window.
pub fn open(parent: &gtk4::Window) {
    let win = gtk4::Window::builder()
        .title(crate::i18n::tr("Taskbar-Einstellungen"))
        .transient_for(parent)
        .modal(false)
        .default_width(880)
        .default_height(520)
        .build();

    // Draggable divider — the column width is the user's choice,
    // not a hard-coded constant.
    let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
    paned.set_wide_handle(true);
    paned.set_shrink_start_child(true);
    paned.set_shrink_end_child(true);
    win.set_child(Some(&paned));

    // Sidebar swapper — "full" shows the bar cards, "back" is the
    // slim strip the menu page collapses to.
    let side_stack = gtk4::Stack::new();
    paned.set_start_child(Some(&side_stack));

    // Left column — bar cards in a scroller plus the add card.
    let left = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    left.set_size_request(240, -1);
    left.set_margin_top(14);
    left.set_margin_bottom(14);
    left.set_margin_start(14);
    left.set_margin_end(8);
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();
    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    scroll.set_child(Some(&list));
    left.append(&scroll);
    let add = gtk4::Button::with_label(&crate::i18n::tr("+  Neue Leiste hinzufügen"));
    add.add_css_class("settings-add");
    left.append(&add);
    side_stack.add_named(&left, Some("full"));

    // "back" strip — the only navigation back to the bars while the
    // menu manager fills the window.
    let back = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    back.set_margin_top(14);
    back.set_margin_start(6);
    back.set_margin_end(6);
    let back_btn = gtk4::Button::new();
    back_btn.set_tooltip_text(Some(&crate::i18n::tr("Zurück zu den Leisten")));
    let back_inner = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    back_inner.append(&gtk4::Image::from_icon_name("go-previous-symbolic"));
    back_inner.append(&gtk4::Label::new(Some(&crate::i18n::tr("Leisten"))));
    back_btn.set_child(Some(&back_inner));
    back.append(&back_btn);
    side_stack.add_named(&back, Some("back"));
    side_stack.set_visible_child_name("full");

    // Right column — the selected bar's detail page inside a
    // scroller: the bar page grew taller than a monitor once the
    // animation section landed, and without scrolling the window
    // would overflow the screen edge with no way to reach the
    // lower controls.
    let detail = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    detail.set_hexpand(true);
    let dscroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .child(&detail)
        .build();
    paned.set_end_child(Some(&dscroll));
    paned.set_position(240);

    // Menu settings card below the bar list — a real settings
    // destination, not a bar, so it lives outside the card list.
    let menu_card = gtk4::Button::with_label(&crate::i18n::tr("Startmenü"));
    menu_card.set_has_frame(false);
    menu_card.set_halign(gtk4::Align::Fill);
    left.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    left.append(&menu_card);

    let ui = Rc::new(Ui {
        win: win.clone(),
        list,
        detail,
        selected: Rc::new(Cell::new(0)),
        menu_page: Rc::new(Cell::new(false)),
        menu_sel: Rc::new(RefCell::new("Audio & Video".to_string())),
        paned,
        side_stack,
        saved_pos: Rc::new(Cell::new(240)),
    });
    {
        let u = ui.clone();
        menu_card.connect_clicked(move |_| set_page(&u, true));
    }
    {
        let u = ui.clone();
        back_btn.connect_clicked(move |_| set_page(&u, false));
    }
    {
        let u = ui.clone();
        add.connect_clicked(move |_| {
            u.selected.set(crate::bars::add());
            rebuild_all(&u);
        });
    }
    rebuild_all(&ui);
    win.present();
}

/// Redraw the card column and the detail page together.
pub fn rebuild_all(ui: &Rc<Ui>) {
    rebuild_list(ui);
    rebuild_detail(ui);
}

/// Switch between bar pages and the start-menu page. Going to the
/// menu collapses the sidebar to the slim back strip and gives the
/// manager the window's width; coming back restores the divider
/// position the user had before.
fn set_page(ui: &Rc<Ui>, menu: bool) {
    if ui.menu_page.get() == menu {
        rebuild_all(ui);
        return;
    }
    ui.menu_page.set(menu);
    if menu {
        ui.saved_pos.set(ui.paned.position().max(120));
        ui.side_stack.set_visible_child_name("back");
        ui.paned.set_position(56);
    } else {
        ui.side_stack.set_visible_child_name("full");
        ui.paned.set_position(ui.saved_pos.get());
    }
    rebuild_all(ui);
}

/// One card per live bar: name + edge hint + delete affordance.
/// Clicking selects; deletion is blocked while a single bar remains.
fn rebuild_list(ui: &Rc<Ui>) {
    while let Some(c) = ui.list.first_child() {
        ui.list.remove(&c);
    }
    let bars = crate::bars::bars();
    let count = bars.borrow().len();
    ui.selected
        .set(ui.selected.get().min(count.saturating_sub(1)));
    for (i, e) in bars.borrow().iter().enumerate() {
        let cfg = e.state.borrow().cfg.clone();
        let card = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        card.add_css_class("settings-card");
        card.set_margin_top(2);
        card.set_margin_bottom(2);
        card.set_margin_start(2);
        card.set_margin_end(2);

        let text = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        text.set_hexpand(true);
        text.set_valign(gtk4::Align::Center);
        let name = gtk4::Label::new(Some(&crate::i18n::trf(
            "Leiste {}",
            &[&(i + 1).to_string()],
        )));
        name.set_halign(gtk4::Align::Start);
        name.add_css_class("card-title");
        let edge = gtk4::Label::new(Some(&edge_name(&cfg.edge)));
        edge.set_halign(gtk4::Align::Start);
        edge.add_css_class("dim-label");
        text.append(&name);
        text.append(&edge);
        card.append(&text);

        // Selected-card highlight driven by a plain CSS class.
        if i == ui.selected.get() {
            card.add_css_class("selected");
        }

        let sel = gtk4::GestureClick::new();
        let u = ui.clone();
        sel.connect_pressed(move |_, _, _, _| {
            u.selected.set(i);
            set_page(&u, false);
        });
        card.add_controller(sel);

        let del = gtk4::Button::from_icon_name("user-trash-symbolic");
        del.set_has_frame(false);
        del.set_valign(gtk4::Align::Center);
        del.set_tooltip_text(Some(&crate::i18n::tr("Leiste entfernen")));
        del.set_sensitive(count > 1);
        // The card claims its own click for selection — Button::clicked
        // never survives that, so the delete action runs on this
        // gesture's RELEASE instead: pressed claims the sequence away
        // from the card, released does the removal itself.
        let dg = gtk4::GestureClick::new();
        dg.set_button(1);
        dg.connect_pressed(|g, _, _, _| {
            g.set_state(gtk4::EventSequenceState::Claimed);
        });
        let (u2, i2) = (ui.clone(), i);
        dg.connect_released(move |_, _, _, _| {
            crate::bars::remove(i2);
            rebuild_all(&u2);
        });
        del.add_controller(dg);
        let _ = &del;
        card.append(&del);
        ui.list.append(&card);
    }
}

/// Swap the detail pane for the selected bar's page.
fn rebuild_detail(ui: &Rc<Ui>) {
    while let Some(c) = ui.detail.first_child() {
        ui.detail.remove(&c);
    }
    if ui.menu_page.get() {
        ui.detail.append(&menu_page::build(ui));
        return;
    }
    let i = ui.selected.get();
    if i < crate::bars::bars().borrow().len() {
        ui.detail.append(&bar_page::build(ui, i));
    }
}

/// Human edge names shown on the bar cards.
fn edge_name(edge: &str) -> String {
    crate::i18n::tr(match edge {
        "top" => "Oben",
        "left" => "Links",
        "right" => "Rechts",
        _ => "Unten",
    })
}
