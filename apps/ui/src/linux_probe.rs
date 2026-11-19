//! Opt-in, content-free diagnostics from the actual GTK window, not its environment.
use nsh_platform_linux::gtk4::{self as gtk, glib, prelude::*};
use std::{cell::Cell, rc::Rc, time::Duration};

pub fn install() {
    let probe = std::env::var_os("ACP_UI_PROBE").is_some();
    let mut last = String::new();
    let attached = Rc::new(Cell::new(false));
    glib::timeout_add_local(Duration::from_millis(50), move || {
        for window in gtk::Window::list_toplevels() {
            let Some(window) = window.downcast_ref::<gtk::Window>() else {
                continue;
            };
            let state = format!(
                "ACP_GTK backend={} renderer={} mapped={} width={} height={} decorated={}",
                gtk::prelude::WidgetExt::display(window).type_().name(),
                window
                    .renderer()
                    .map(|r| r.type_().name().to_string())
                    .unwrap_or_default(),
                window.is_mapped(),
                window.width(),
                window.height(),
                window.is_decorated()
            );
            if let Some(renderer) = window.renderer() {
                let _ = acp_ui::DESKTOP_RUNTIME.set((
                    gtk::prelude::WidgetExt::display(window)
                        .type_()
                        .name()
                        .to_string(),
                    renderer.type_().name().to_string(),
                ));
            }
            if probe && state != last {
                eprintln!("{state} elapsed={:.6}", acp_ui::probe_elapsed());
                last = state;
            }
            if probe
                && !attached.replace(true)
                && let Some(clock) = window.frame_clock()
            {
                let count = Cell::new(0);
                clock.connect_after_paint(move |_| {
                    let n = count.get() + 1;
                    count.set(n);
                    if n <= 20 {
                        eprintln!(
                            "ACP_GTK_FRAME after_paint={n} elapsed={:.6}",
                            acp_ui::probe_elapsed()
                        );
                    }
                });
            }
        }
        glib::ControlFlow::Continue
    });
}
