use acp_operator_model::{Model, Phase, Preference, native::Inputs};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Render, Role, SharedString, Window,
    WindowBounds, WindowOptions,
    component::{
        button::*,
        scroll::ScrollableElement,
        table::{Column, DataTable, TableDelegate, TableState},
        *,
    },
    div,
    prelude::*,
    px, size,
};

struct Preferences {
    rows: Vec<Preference>,
}
impl TableDelegate for Preferences {
    fn columns_count(&self, _: &App) -> usize {
        4
    }
    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }
    fn column(&self, index: usize, _: &App) -> Column {
        let names = ["Key", "Value", "Scope", "Source"];
        Column::new(names[index], names[index]).width(px(160.))
    }
    fn render_td(
        &mut self,
        row: usize,
        col: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let r = &self.rows[row];
        let value = match col {
            0 => &r.key,
            1 => &r.value,
            2 => &r.scope,
            _ => &r.source,
        };
        div().child(SharedString::from(value.clone()))
    }
}
struct Console {
    model: Model,
    inputs: Inputs,
    table: Entity<TableState<Preferences>>,
    ready_reported: bool,
    #[cfg(feature = "hybrid")]
    embedded: Entity<nsh_platform_gpui::AppView>,
}
impl Console {
    fn new(inputs: Inputs, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut model = Model::loading();
        model.live = inputs.runtime_dir.is_some();
        if inputs.runtime_dir.is_none() {
            model.receive(inputs.fetch());
        } else {
            model.phase = Phase::Unavailable;
        }
        let rows = model.rows().into_iter().cloned().collect();
        let table = cx.new(|cx| TableState::new(Preferences { rows }, window, cx));
        #[cfg(feature = "hybrid")]
        acp_operator_ui::INPUTS.set(inputs.clone()).ok();
        #[cfg(feature = "hybrid")]
        let embedded = crate::hybrid::surface(window, cx);
        let needs_load = inputs.runtime_dir.is_some();
        let mut view = Self {
            model,
            inputs,
            table,
            ready_reported: false,
            #[cfg(feature = "hybrid")]
            embedded,
        };
        if needs_load {
            view.reconnect(cx);
        }
        view
    }
    fn update_table(&mut self, cx: &mut Context<Self>) {
        let rows = if self.model.phase == Phase::Ready {
            self.model.rows().into_iter().cloned().collect()
        } else {
            Vec::new()
        };
        self.table.update(cx, |t, cx| {
            t.delegate_mut().rows = rows;
            cx.notify();
        });
        cx.notify();
    }
    fn reconnect(&mut self, cx: &mut Context<Self>) {
        if self.model.phase == Phase::Loading {
            return;
        }
        self.model.phase = Phase::Loading;
        self.update_table(cx);
        let inputs = self.inputs.clone();
        let job = cx
            .background_executor()
            .spawn(async move { inputs.fetch() });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.model.receive(result);
                this.update_table(cx);
            });
        })
        .detach();
    }
}
impl Render for Console {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.model.phase == Phase::Ready && !self.ready_reported {
            if std::env::var_os("ACP_UI_PROBE").is_some() {
                eprintln!("ACP_UI_READY");
            }
            self.ready_reported = true;
        }
        let projects = self
            .model
            .data
            .as_ref()
            .map(|s| {
                s.projects
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut sidebar = div()
            .v_flex()
            .w(px(190.))
            .gap_2()
            .p_3()
            .child("Allowlisted projects");
        for (index, name) in projects.into_iter().enumerate() {
            sidebar = sidebar.child(Button::new(("project", index)).label(name).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.model.select(index);
                    this.update_table(cx);
                }),
            ));
        }
        let detail = div()
            .id("detail-scroll")
            .v_flex()
            .flex_1()
            .min_w_0()
            .gap_2()
            .p_3()
            .overflow_y_scrollbar()
            .child(
                div()
                    .id("daemon-state")
                    .role(Role::Status)
                    .aria_label(SharedString::from(self.model.status()))
                    .child(SharedString::from(self.model.status())),
            )
            .child(
                div()
                    .h(px(170.))
                    .child(DataTable::new(&self.table).stripe(true).bordered(true)),
            )
            .children(
                self.model
                    .detail_lines()
                    .into_iter()
                    .map(|line| div().child(SharedString::from(line))),
            );
        let main = div()
            .h_flex()
            .items_start()
            .flex_1()
            .min_h_0()
            .child(sidebar)
            .child(detail);
        #[cfg(feature = "hybrid")]
        let main = div()
            .v_flex()
            .flex_1()
            .min_h_0()
            .child(main.h(px(240.)).flex_none())
            .child(
                div()
                    .h(px(300.))
                    .w_full()
                    .flex_shrink_0()
                    .v_flex()
                    .child("Embedded NSH Rust surface · same read-only model")
                    .child(div().flex_1().min_h_0().child(self.embedded.clone())),
            );
        div()
            .v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .p_3()
                    .text_lg()
                    .child("Agent Control Plane · GPUI Kit experiment"),
            )
            .child("Synthetic/offline unless explicit private IPC flags supplied · no writes")
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap_2()
                    .p_2()
                    .child(
                        Button::new("reconnect")
                            .label("Reconnect")
                            .on_click(cx.listener(|this, _, _, cx| this.reconnect(cx))),
                    )
                    .child(
                        Button::new("next")
                            .label("Next project")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.model.next();
                                this.update_table(cx);
                            })),
                    )
                    .child(
                        Button::new("theme")
                            .label("Light / dark")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.model.dark = !this.model.dark;
                                Theme::change(
                                    if this.model.dark {
                                        ThemeMode::Dark
                                    } else {
                                        ThemeMode::Light
                                    },
                                    Some(window),
                                    cx,
                                );
                                cx.notify();
                            })),
                    )
                    .child(Button::new("filter").label("Filter commit / all").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.model.filter = if this.model.filter.is_empty() {
                                "commit".into()
                            } else {
                                String::new()
                            };
                            this.update_table(cx);
                        }),
                    ))
                    .child(
                        Button::new("states")
                            .label("Demo state")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.model.phase = match this.model.phase {
                                    Phase::Ready => Phase::Loading,
                                    Phase::Loading => Phase::Empty,
                                    Phase::Empty => Phase::Error,
                                    Phase::Error => Phase::Unavailable,
                                    Phase::Unavailable => Phase::Ready,
                                };
                                this.update_table(cx);
                            })),
                    ),
            )
            .child(main)
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;
    use gpui_kit::TestAppContext;
    use gpui_kit::test::TestWindowExt;
    #[gpui_kit::test]
    fn component_selection_filter_theme_and_error_states(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (handle, view) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Console::new(Inputs::default(), window, cx))
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(view.read(cx).model.project().unwrap().name, "alpha");
            window.click("next", cx);
            assert_eq!(view.read(cx).model.project().unwrap().name, "beta");
            window.focus_next(cx);
            let focused = window.focused(cx);
            assert!(focused.is_some());
            window.focus_next(cx);
            assert_ne!(window.focused(cx), focused);
            window.click("theme", cx);
            assert!(view.read(cx).model.dark);
            window.click("filter", cx);
            assert_eq!(view.read(cx).model.filter, "commit");
            for phase in [
                Phase::Loading,
                Phase::Empty,
                Phase::Error,
                Phase::Unavailable,
                Phase::Ready,
            ] {
                window.click("states", cx);
                assert_eq!(view.read(cx).model.phase, phase);
            }
            window.press("tab", cx);
            assert!(window.find("reconnect").bounds().size.width > px(0.));
        })
        .unwrap();
    }
}
#[cfg(feature = "hybrid")]
mod hybrid;
fn main() {
    let inputs = Inputs::parse();
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1080.), px(760.)), cx)),
                ..Default::default()
            },
            cx,
            |window, cx| {
                window.set_window_title("ACP GPUI Kit — synthetic read-only spike");
                cx.new(|cx| Console::new(inputs.clone(), window, cx))
            },
        )
        .expect("Native window could not open");
        cx.activate(true);
    });
}
