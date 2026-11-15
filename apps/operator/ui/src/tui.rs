use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use nsh_platform_tui::{App, AppOptions, AppView, Input, Modifiers, Root};
use ratatui_core::terminal::Terminal;
use ratatui_crossterm::CrosstermBackend;
use std::{io, time::Duration};
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen);
    }
}
fn main() -> io::Result<()> {
    acp_operator_ui::TERMINAL_SURFACE.store(true, std::sync::atomic::Ordering::Relaxed);
    acp_operator_ui::PROBE_STARTED
        .set(std::time::Instant::now())
        .ok();
    let preview = acp_operator_ui::brand_preview::configure().map_err(io::Error::other)?;
    if !preview {
        acp_operator_ui::INPUTS
            .set(acp_operator_model::native::Inputs::parse())
            .ok();
    }
    terminal::enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut options = AppOptions::default();
    options.storage = Some(acp_operator_ui::ephemeral_storage());
    options.cache = nsh_platform_tui::CacheOptions::off();
    let mut view = AppView::new(options);
    view.load(App::Rust(if preview {
        Root::of::<acp_operator_ui::brand_preview::BrandPreview>()
    } else {
        Root::of::<acp_operator_ui::App>()
    }))?;
    loop {
        view.draw_terminal(&mut terminal)?;
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    if k.code == KeyCode::Char('q') && k.modifiers.contains(KeyModifiers::CONTROL)
                        || (k.code == KeyCode::Char('c')
                            && k.modifiers.contains(KeyModifiers::CONTROL))
                    {
                        break;
                    }
                    let key = match k.code {
                        KeyCode::Tab | KeyCode::BackTab => "Tab".into(),
                        KeyCode::Enter => "Enter".into(),
                        KeyCode::Esc => "Escape".into(),
                        KeyCode::Up => "ArrowUp".into(),
                        KeyCode::Down => "ArrowDown".into(),
                        KeyCode::Left => "ArrowLeft".into(),
                        KeyCode::Right => "ArrowRight".into(),
                        KeyCode::Backspace => "Backspace".into(),
                        KeyCode::Delete => "Delete".into(),
                        KeyCode::Home => "Home".into(),
                        KeyCode::End => "End".into(),
                        KeyCode::Char(c) => c.to_string(),
                        _ => continue,
                    };
                    view.handle(Input::Key {
                        key,
                        modifiers: Modifiers {
                            ctrl: k.modifiers.contains(KeyModifiers::CONTROL),
                            alt: k.modifiers.contains(KeyModifiers::ALT),
                            shift: k.modifiers.contains(KeyModifiers::SHIFT)
                                || k.code == KeyCode::BackTab,
                            cmd: k.modifiers.contains(KeyModifiers::SUPER),
                        },
                        release: false,
                    });
                }
                Event::Paste(s) => {
                    view.handle(Input::Paste(s));
                }
                // ConPTY can reflow previously emitted cells before this event arrives.
                // Reset the terminal diff buffer after the resize notification so the
                // next frame paints the whole current viewport, including unchanged text.
                Event::Resize(_, _) => terminal.clear()?,
                _ => {}
            }
        }
        if view.exit_requested() {
            break;
        }
    }
    view.stop();
    Ok(())
}
