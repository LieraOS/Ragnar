//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! Print support: paginates the buffer's plain text into fixed
//! line-height pages rendered through Pango/Cairo.

use gtk4::prelude::*;

/// Runs the print dialog for the current buffer text.
pub fn run(parent: &gtk4::Window, text: String, title: &str) {
    let op = gtk4::PrintOperation::new();
    op.set_job_name(title);
    op.set_embed_page_setup(true);

    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let lines = std::rc::Rc::new(std::cell::RefCell::new(lines));

    {
        let lines = lines.clone();
        op.connect_begin_print(move |op, ctx| {
            let w = ctx.width();
            let h = ctx.height();
            let layout = ctx.create_pango_layout();
            layout.set_font_description(Some(&gtk4::pango::FontDescription::from_string(
                "Monospace 9",
            )));
            layout.set_width((w * gtk4::pango::SCALE as f64) as i32);
            layout.set_text("Mg");
            let (_, lh) = layout.pixel_size();
            let per = (h / lh.max(1) as f64) as i32 - 2;
            let n = lines.borrow().len().max(1) as i32;
            op.set_n_pages((n + per - 1) / per.max(1));
        });
    }
    {
        let lines = lines.clone();
        op.connect_draw_page(move |_op, ctx, page| {
            let w = ctx.width();
            let h = ctx.height();
            let layout = ctx.create_pango_layout();
            layout.set_font_description(Some(&gtk4::pango::FontDescription::from_string(
                "Monospace 9",
            )));
            layout.set_width((w * gtk4::pango::SCALE as f64) as i32);
            layout.set_text("Mg");
            let (_, lh) = layout.pixel_size();
            let per = ((h / lh.max(1) as f64) as i32 - 2).max(1) as usize;
            let cr = ctx.cairo_context();
            cr.set_source_rgb(0.0, 0.0, 0.0);
            let ls = lines.borrow();
            for (row, l) in ls.iter().skip(page as usize * per).take(per).enumerate() {
                cr.move_to(0.0, (row * lh as usize) as f64);
                layout.set_text(l);
                pangocairo::functions::show_layout(&cr, &layout);
            }
        });
    }
    let _ = op.run(gtk4::PrintOperationAction::PrintDialog, Some(parent));
}
