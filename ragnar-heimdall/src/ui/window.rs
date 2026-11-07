//! Main window: a fixed sidebar (like the classic Heimdall layout)
//! next to a `Stack` of content views, plus a header refresh button.

use std::cell::Cell;
use std::rc::Rc;

use libadwaita as adw;

use adw::prelude::*;
use adw::{Application, ApplicationWindow, HeaderBar, ToolbarView};
use gtk4::{Box, Button, Orientation, ScrolledWindow, Separator, Stack};

use super::sidebar::{self, View};

/// Sidebar width matching the classic layout's proportions.
const SIDEBAR_WIDTH: i32 = 220;

/// The application window.
pub struct MainWindow {
    window: ApplicationWindow,
    stack: Stack,
    /// Guards against recursive sidebar callbacks during refresh.
    refreshing: Rc<Cell<bool>>,
}

impl MainWindow {
    /// Create the window bound to `app`.
    pub fn new(app: &Application) -> Rc<Self> {
        let stack = Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .build();

        let sidebar = sidebar::build({
            let stack = stack.clone();
            move |view| stack.set_visible_child_name(view.tag())
        });
        sidebar.set_size_request(SIDEBAR_WIDTH, -1);

        let content = Box::new(Orientation::Horizontal, 0);
        content.append(&sidebar);
        content.append(&Separator::new(Orientation::Vertical));
        content.append(&stack);
        stack.set_hexpand(true);

        let refresh = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(gettextrs::gettext("Refresh"))
            .build();
        let header = HeaderBar::new();
        header.pack_end(&refresh);

        let toolbar = ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));

        let window = ApplicationWindow::builder()
            .application(app)
            .title(gettextrs::gettext("Heimdall"))
            .content(&toolbar)
            .default_width(1024)
            .default_height(640)
            .build();

        let this = Rc::new(Self {
            window,
            stack,
            refreshing: Rc::new(Cell::new(false)),
        });
        this.populate();
        this.wire(refresh);
        // Select the landing view only after the pages exist.
        sidebar.select_row(sidebar.row_at_index(0).as_ref());
        this.stack.set_visible_child_name(View::Dashboard.tag());
        this
    }

    /// Present the window.
    pub fn present(&self) {
        self.window.present();
    }

    /// (Re)build every stack page from live data.
    fn populate(&self) {
        self.set_page(View::Dashboard.tag(), super::dashboard::build());
        self.set_page(View::Interfaces.tag(), super::interfaces::build());
        self.set_page(View::Routing.tag(), super::routing::build());
        self.set_page(View::Dns.tag(), super::dns::build());
        self.set_page(View::Vpn.tag(), super::vpn::build(&self.stack));
        self.set_page(View::Firewall.tag(), super::firewall::build(&self.stack));
        self.set_page(View::Diagnostics.tag(), super::diagnostics::build());
        for view in View::ALL {
            if matches!(
                view,
                View::Dashboard
                    | View::Interfaces
                    | View::Routing
                    | View::Dns
                    | View::Vpn
                    | View::Firewall
                    | View::Diagnostics
            ) {
                continue;
            }
            let title = gettextrs::gettext(view.label());
            self.set_page(view.tag(), super::placeholder::build(&title).upcast());
        }
    }

    /// Replace a stack page, keeping it scrolled consistently.
    fn set_page(&self, tag: &str, widget: gtk4::Widget) {
        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&widget)
            .vexpand(true)
            .build();
        if let Some(old) = self.stack.child_by_name(tag) {
            self.stack.remove(&old);
        }
        self.stack.add_named(&scroll, Some(tag));
    }

    /// Refresh rebuilds data views; selection/page stays untouched.
    fn wire(self: &Rc<Self>, refresh: Button) {
        let this = Rc::clone(self);
        refresh.connect_clicked(move |_| {
            if this.refreshing.replace(true) {
                return;
            }
            this.populate();
            this.refreshing.set(false);
        });
    }
}
