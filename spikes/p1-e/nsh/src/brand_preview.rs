//! B0 exploration only. No IPC inputs, filesystem reads, persistence or final brand.
use nsh_base::button;
use nsh_core::prelude::*;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Instrument,
    Ledger,
    Harbor,
}
impl Direction {
    pub const ALL: [Self; 3] = [Self::Instrument, Self::Ledger, Self::Harbor];
    pub fn label(self) -> &'static str {
        match self {
            Self::Instrument => "A · Instrument",
            Self::Ledger => "B · Ledger",
            Self::Harbor => "C · Harbor",
        }
    }
    fn caption(self) -> &'static str {
        match self {
            Self::Instrument => "Precise / compact / instrument-like",
            Self::Ledger => "Warm / editorial / evidence-led",
            Self::Harbor => "Calm / layered / workspace-like",
        }
    }
    fn palette(self) -> Swatch {
        match self {
            Self::Instrument => Swatch {
                background: 0x111718,
                panel: 0x1b2425,
                inset: 0x263334,
                text: 0xeaf2ee,
                muted: 0xa6bab2,
                border: 0x738d83,
                accent: 0x9ee6be,
                on_accent: 0x10231b,
                warning: 0xf3cc86,
                radius: 2.0,
            },
            Self::Ledger => Swatch {
                background: 0xf4f0e7,
                panel: 0xfffcf5,
                inset: 0xe8e1d3,
                text: 0x302b25,
                muted: 0x695e50,
                border: 0x8b7e6a,
                accent: 0x8e3e26,
                on_accent: 0xfffcf5,
                warning: 0x805005,
                radius: 0.0,
            },
            Self::Harbor => Swatch {
                background: 0x131c30,
                panel: 0x202d46,
                inset: 0x2c3b56,
                text: 0xedf2fc,
                muted: 0xb3c2db,
                border: 0x7c91b4,
                accent: 0xb5c8ff,
                on_accent: 0x182445,
                warning: 0xffd394,
                radius: 12.0,
            },
        }
    }
}
#[derive(Clone, Copy)]
struct Swatch {
    background: u32,
    panel: u32,
    inset: u32,
    text: u32,
    muted: u32,
    border: u32,
    accent: u32,
    on_accent: u32,
    warning: u32,
    radius: f32,
}
fn color(value: u32) -> AdaptiveColor {
    AdaptiveColor::rgb(value, value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub direction: Direction,
    pub handoff: bool,
}
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Preview accepts a closed vocabulary only, never location/auth/project inputs.
pub fn parse(args: &[String]) -> Result<Option<Config>, &'static str> {
    if !args.iter().any(|a| a == "--brand-preview") {
        return Ok(None);
    }
    let mut config = Config {
        direction: Direction::Instrument,
        handoff: false,
    };
    let mut direction_seen = false;
    let mut page_seen = false;
    for pair in args.chunks(2) {
        if pair.len() != 2 {
            return Err("Preview requires flag/value pairs");
        }
        match pair[0].as_str() {
            "--brand-preview" if !direction_seen => {
                config.direction = match pair[1].as_str() {
                    "a" => Direction::Instrument,
                    "b" => Direction::Ledger,
                    "c" => Direction::Harbor,
                    _ => return Err("Choose preview a, b or c"),
                };
                direction_seen = true;
            }
            "--preview-page" if !page_seen => {
                config.handoff = match pair[1].as_str() {
                    "dashboard" => false,
                    "handoff" => true,
                    _ => return Err("Preview supports dashboard or handoff only"),
                };
                page_seen = true;
            }
            _ => return Err("Brand preview permits synthetic data only; no other inputs"),
        }
    }
    Ok(Some(config))
}
pub fn configure() -> Result<bool, &'static str> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let Some(config) = parse(&args)? else {
        return Ok(false);
    };
    CONFIG
        .set(config)
        .map_err(|_| "Preview already configured")?;
    Ok(true)
}
fn compact() -> bool {
    super::tokens::compact()
}
fn terminal() -> bool {
    super::TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed)
}

fn choice(
    direction: Signal<Direction>,
    label: &'static str,
    selected: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    let selected = computed(selected);
    button()
        .px_2()
        .py_1()
        .border_2()
        .rounded(move || px(direction.get().palette().radius.min(6.0)))
        .accessibility_label(label)
        .selected(move || selected.get())
        .bg(move || {
            let p = direction.get().palette();
            color(if selected.get() { p.accent } else { p.inset })
        })
        .text_color(move || {
            let p = direction.get().palette();
            color(if selected.get() { p.on_accent } else { p.text })
        })
        .border_color(move || color(direction.get().palette().border))
        .focus(|s| s.border_color(color(direction.get().palette().text)))
        .hover(|s| s.border_color(color(direction.get().palette().accent)))
        .on_press(callback)
        .child(text(label))
}
fn heading(direction: Direction, title: &'static str) -> impl IntoElement {
    text(title)
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(if terminal() {
            14.0
        } else if direction == Direction::Ledger {
            26.0
        } else {
            22.0
        }))
        .font_family(if direction == Direction::Ledger {
            "serif"
        } else {
            "sans-serif"
        })
}
fn label(p: Swatch, value: impl Into<String>) -> impl IntoElement + Styled {
    text(value.into())
        .text_size(px(12.0))
        .text_color(color(p.muted))
}
fn panel(direction: Direction, title: &'static str, lines: &[&str]) -> impl IntoElement {
    let p = direction.palette();
    view()
        .p_3()
        .gap_2()
        .style_when(compact, |s| s.p_2().gap_1())
        .style_when(terminal, |s| s.p_1().gap_1())
        .rounded(px(p.radius))
        .border_1()
        .border_color(color(p.border))
        .bg(color(p.panel))
        .child(
            text(title)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color(p.accent)),
        )
        .children(lines.iter().map(|line| text((*line).to_string())))
}
fn facts(direction: Direction) -> impl IntoElement {
    let p = direction.palette();
    view()
        .gap_2()
        .p_3()
        .style_when(compact, |s| s.p_2().gap_1())
        .style_when(terminal, |s| s.p_1().gap_1())
        .rounded(px(p.radius))
        .border_1()
        .border_color(color(p.border))
        .bg(color(p.panel))
        .child(label(p, "REPOSITORY / observed synthetic fixture"))
        .child(
            text("alpha")
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(20.0)),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(text("Branch  main").font_family("monospace"))
                .child(text("HEAD  Unknown").font_family("monospace")),
        )
        .child(text("Worktree  Unknown").text_color(color(p.warning)))
        .child(label(
            p,
            "No clean-state or current-test claim. Refresh in the live console.",
        ))
}
fn dashboard(direction: Direction, handoff: Signal<bool>) -> impl IntoElement {
    let p = direction.palette();
    let facts = facts(direction).into_element();
    let preferences = panel(
        direction,
        "03 / Confirmed preferences",
        &[
            "3 effective records · alpha",
            "interaction.language   zh-CN",
            "Scope: global · Source: synthetic fixture",
        ],
    )
    .into_element();
    let mut content = view().gap_3().style_when(compact, |s| s.gap_1());
    if direction == Direction::Harbor {
        content = content.child(
            view()
                .flex_row()
                .gap_3()
                .style_when(compact, |s| s.flex_col())
                .child(view().flex_1().child(facts))
                .child(view().flex_1().child(preferences)),
        );
    } else {
        content = content.child(facts).child(preferences);
    }
    content
        .child(panel(
            direction,
            "04 / Resume with context",
            &[
                "Task: ui-density-review · exact ID",
                "Recorded evidence belongs to an earlier session.",
                "Current fingerprint is unavailable; inspect before resuming.",
            ],
        ))
        .child(choice(
            signal(direction),
            "Open Handoff",
            || false,
            move || handoff.set(true),
        ))
        .child(label(
            p,
            "Read-only context and policy. No agent launch or automatic writes.",
        ))
}
fn handoff_page(direction: Direction) -> impl IntoElement {
    let p = direction.palette();
    view()
        .gap_3()
        .style_when(terminal, |s| s.gap_1())
        .child(label(p, "EXACT TASK / alpha / ui-density-review"))
        .child(panel(
            direction,
            "Objective",
            &[
                "Refine the compact operator console.",
                "Keep scope and preference provenance visible.",
            ],
        ))
        .child(panel(
            direction,
            "Next action",
            &[
                "Compare the synthetic wide and compact layouts.",
                "Choose a brand direction before the five-page implementation.",
            ],
        ))
        .child(
            view()
                .p_3()
                .gap_1()
                .style_when(terminal, |s| s.p_1())
                .border_2()
                .border_color(color(p.warning))
                .rounded(px(p.radius))
                .child(
                    text("REVIEW NEEDED / Current repository uncertainty")
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(color(p.warning)),
                )
                .child(text("HEAD and worktree fingerprint: Unknown"))
                .child(label(
                    p,
                    "Earlier checks do not establish current verification.",
                )),
        )
        .child(panel(
            direction,
            "Recorded validation / historical",
            &[
                "PASS · synthetic-layout · earlier revision",
                "NOT_RUN · current validation · this preview",
                "Evidence is illustrative. No real check was run by this screen.",
            ],
        ))
}
fn page(direction: Direction, handoff: Signal<bool>) -> impl IntoElement {
    let p = direction.palette();
    let title = if handoff.get() {
        "Handoff"
    } else {
        "Dashboard"
    };
    let mut body = view().gap_3().style_when(compact, |s| s.gap_1()).child(
        view()
            .gap_1()
            .child(
                label(
                    p,
                    if handoff.get() {
                        "04 / CONTINUITY"
                    } else {
                        "01 / PROJECT CONTEXT"
                    },
                )
                .style_when(compact, |s| s.hidden()),
            )
            .child(heading(direction, title))
            .child(label(p, direction.caption()).style_when(compact, |s| s.hidden())),
    );
    if handoff.get() {
        body = body.child(handoff_page(direction));
    } else {
        body = body.child(dashboard(direction, handoff));
    }
    body
}

#[component]
pub fn BrandPreview() -> impl IntoElement {
    preview(CONFIG.get().copied().unwrap_or(Config {
        direction: Direction::Instrument,
        handoff: false,
    }))
}
fn preview(config: Config) -> impl IntoElement {
    let direction = signal(config.direction);
    let handoff = signal(config.handoff);
    let always = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    for (key, d) in [
        ("alt+a", Direction::Instrument),
        ("alt+b", Direction::Ledger),
        ("alt+c", Direction::Harbor),
    ] {
        on_key_with(key, always, move |_| direction.set(d));
    }
    on_key_with("alt+1", always, move |_| handoff.set(false));
    on_key_with("alt+4", always, move |_| handoff.set(true));
    view()
        .flex_1()
        .p_4()
        .gap_3()
        .style_when(compact, |s| s.p_2().gap_1())
        .text_size(px(14.0))
        .font_family("sans-serif")
        .bg(move || color(direction.get().palette().background))
        .text_color(move || color(direction.get().palette().text))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .children(Direction::ALL.into_iter().map(move |d| {
                    choice(
                        direction,
                        d.label(),
                        move || direction.get() == d,
                        move || direction.set(d),
                    )
                })),
        )
        .child(
            text("B0 EXPLORATION · OFFLINE_SYNTHETIC · BRAND SELECTION PENDING")
                .text_size(px(12.0))
                .text_color(move || color(direction.get().palette().muted)),
        )
        .child(
            view()
                .flex_row()
                .flex_1()
                .gap_3()
                .style_when(compact, |s| s.flex_col().gap_1())
                .child(
                    view()
                        .gap_3()
                        .style_when(compact, |s| s.flex_row().items_center().gap_3())
                        .w(move || {
                            px(if compact() {
                                (window().width.0 - 16.0).max(1.0)
                            } else {
                                196.0
                            })
                        })
                        .child(dynamic(move || {
                            let d = direction.get();
                            let p = d.palette();
                            view()
                                .gap_1()
                                .child(
                                    text(match d {
                                        Direction::Instrument => "[ ACP ]",
                                        Direction::Ledger => "ACP /",
                                        Direction::Harbor => "= ACP",
                                    })
                                    .font_family(if d == Direction::Ledger {
                                        "serif"
                                    } else {
                                        "monospace"
                                    })
                                    .text_size(px(if compact() { 20.0 } else { 28.0 }))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(color(p.accent)),
                                )
                                .child(
                                    label(p, "Agent Control Plane")
                                        .style_when(compact, |s| s.hidden()),
                                )
                        }))
                        .child(
                            view()
                                .gap_2()
                                .style_when(compact, |s| s.flex_row().gap_1())
                                .child(choice(
                                    direction,
                                    "1 Dashboard",
                                    move || !handoff.get(),
                                    move || handoff.set(false),
                                ))
                                .child(choice(
                                    direction,
                                    "4 Handoff",
                                    move || handoff.get(),
                                    move || handoff.set(true),
                                )),
                        )
                        .child(
                            text("Synthetic scope / alpha\nRead-only preview\nNo daemon connected")
                                .text_size(px(12.0))
                                .style_when(compact, |s| s.hidden())
                                .text_color(move || color(direction.get().palette().muted)),
                        ),
                )
                .child(
                    scroll()
                        .flex_1()
                        .child(dynamic(move || page(direction.get(), handoff))),
                ),
        )
        .child(
            text("Alt A/B/C direction · Alt 1/4 page · TUI Ctrl Q closes")
                .text_size(px(12.0))
                .text_color(move || color(direction.get().palette().muted)),
        )
        .on_layout(move |_: &Bounds<Points>| {
            if std::env::var_os("ACP_UI_PROBE").is_some() {
                eprintln!(
                    "ACP_NSH_LAYOUT_READY brand_preview=true elapsed={:.6}",
                    super::probe_elapsed()
                );
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_cannot_accept_live_or_arbitrary_inputs() {
        let args = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse(&args(&[
                "--brand-preview",
                "b",
                "--preview-page",
                "handoff"
            ])),
            Ok(Some(Config {
                direction: Direction::Ledger,
                handoff: true
            }))
        );
        for extra in [
            "--runtime-dir",
            "--endpoint",
            "--wsl-distribution",
            "--project",
            "--synthetic-rows",
            "--brand-preview",
            "--unknown",
        ] {
            assert!(parse(&args(&["--brand-preview", "a", extra, "x"])).is_err());
        }
        assert!(
            parse(&args(&[
                "--brand-preview",
                "a",
                "--preview-page",
                "preferences"
            ]))
            .is_err()
        );
        assert!(parse(&args(&["--brand-preview"])).is_err());
    }
    #[component]
    fn TestPreview() -> impl IntoElement {
        preview(Config {
            direction: Direction::Instrument,
            handoff: false,
        })
    }
    #[test]
    fn candidate_navigation_keeps_synthetic_historical_semantics() {
        let mut app =
            nsh_testing::Headless::start(nsh_engine::EngineConfig::new(nsh_engine::Root::of::<
                TestPreview,
            >()))
            .unwrap();
        app.wait_for("B0 EXPLORATION · OFFLINE_SYNTHETIC · BRAND SELECTION PENDING")
            .unwrap();
        app.press("B · Ledger").unwrap();
        app.wait_for("ACP /").unwrap();
        app.press_key("alt+4").unwrap();
        app.wait_for("Earlier checks do not establish current verification.")
            .unwrap();
        app.press("C · Harbor").unwrap();
        app.wait_for("= ACP").unwrap();
        app.press("1 Dashboard").unwrap();
        app.wait_for("3 effective records · alpha").unwrap();
        assert!(app.screen().clipboard().is_none());
    }
}
