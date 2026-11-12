//! Same-binary interactive entry. Terminal restoration is RAII-backed, including unwinding.
pub mod app;
pub mod data;
#[cfg(test)]
mod tests;
pub mod view;

use crate::failure::Failure;
use app::{Action, App, Event, Screen};
use crossterm::{
    event::{self, Event as TerminalEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, IsTerminal},
    sync::mpsc,
    time::{Duration, Instant},
};

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            event::DisableBracketedPaste,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}
fn terminal_error(error: impl std::fmt::Display) -> Failure {
    Failure::new(
        format!("interactive terminal: {error}"),
        "use an interactive terminal, or run constitution --help for non-interactive commands",
    )
}
pub fn run() -> Result<(), Failure> {
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        return Err(terminal_error("stdin and stdout must both be terminals"));
    }
    let mut app = App::default();
    match data::presets() {
        Ok(presets) => {
            app::update(&mut app, Event::Presets(presets));
            app::update(&mut app, Event::SelectPreset(0));
        }
        Err(e) => app.error = Some(e),
    }
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            event::DisableBracketedPaste,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        previous_hook(info);
    }));
    enable_raw_mode().map_err(terminal_error)?;
    let _restore = Restore;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        event::EnableBracketedPaste
    )
    .map_err(terminal_error)?;
    let mut terminal =
        Terminal::new(CrosstermBackend::new(io::stdout())).map_err(terminal_error)?;
    let (tx, rx) = mpsc::channel();
    let mut workers = data::Workers::default();
    app::update(&mut app, Event::Clock(data::unix_ns()));
    for action in app::update(&mut app, Event::Refresh) {
        workers.dispatch(action, &tx);
    }
    let mut refreshed = Instant::now();
    while !app.quit {
        app::update(&mut app, Event::Clock(data::unix_ns()));
        while let Ok(event) = rx.try_recv() {
            for action in app::update(&mut app, event) {
                workers.dispatch(action, &tx);
            }
        }
        terminal
            .draw(|f| view::render(f, f.area(), &app))
            .map_err(terminal_error)?;
        if event::poll(Duration::from_millis(100)).map_err(terminal_error)? {
            match event::read().map_err(terminal_error)? {
                TerminalEvent::Key(key) if key.kind == KeyEventKind::Press => {
                    for action in key_event(&mut app, key) {
                        workers.dispatch(action, &tx);
                    }
                }
                TerminalEvent::Paste(text) => {
                    // Only text fields accept paste; terminal control characters are not retained.
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    if app.wizard.editing {
                        app.wizard.field_mut().push_str(&text);
                        app.wizard.confirmation = None;
                    } else if app.receipt_editing {
                        app.receipt_id
                            .extend(text.chars().filter(char::is_ascii_digit));
                    } else if app.receipt_save_editing {
                        app.receipt_path.push_str(&text);
                    }
                }
                _ => {}
            }
        }
        if refreshed.elapsed() >= Duration::from_secs(30) {
            for action in app::update(&mut app, Event::Refresh) {
                workers.dispatch(action, &tx);
            }
            refreshed = Instant::now();
        }
    }
    Ok(())
}
fn key_event(app: &mut App, key: KeyEvent) -> Vec<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        if app.running {
            return app::update(app, Event::Cancel);
        }
        app.quit = true;
        return Vec::new();
    }
    if app.help {
        return match key.code {
            KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => app::update(app, Event::Back),
            _ => Vec::new(),
        };
    }
    if app.confirm_cancel {
        return match key.code {
            KeyCode::Char('y') => app::update(app, Event::ConfirmCancel),
            KeyCode::Esc | KeyCode::Char('n') => app::update(app, Event::Back),
            _ => Vec::new(),
        };
    }
    if key.code == KeyCode::Esc {
        return app::update(app, Event::Back);
    }
    if app.wizard.editing {
        match key.code {
            KeyCode::Enter => app.wizard.editing = false,
            KeyCode::Tab => app.wizard.field = (app.wizard.field + 1) % 6,
            KeyCode::BackTab => app.wizard.field = (app.wizard.field + 5) % 6,
            KeyCode::Backspace => {
                app.wizard.field_mut().pop();
            }
            KeyCode::Char(c) if !c.is_control() => app.wizard.field_mut().push(c),
            _ => {}
        }
        app.wizard.confirmation = None;
        return Vec::new();
    }
    if app.receipt_editing || app.receipt_save_editing {
        let saving = app.receipt_save_editing;
        match key.code {
            KeyCode::Enter => {
                app.receipt_editing = false;
                app.receipt_save_editing = false;
                return app::update(
                    app,
                    if saving {
                        Event::SaveReceipt
                    } else {
                        Event::FetchReceipt
                    },
                );
            }
            KeyCode::Backspace => {
                if saving {
                    app.receipt_path.pop();
                } else {
                    app.receipt_id.pop();
                }
            }
            KeyCode::Char(c) if saving && !c.is_control() => app.receipt_path.push(c),
            KeyCode::Char(c) if c.is_ascii_digit() => app.receipt_id.push(c),
            _ => {}
        }
        return Vec::new();
    }
    let event = match key.code {
        KeyCode::Char('?') => Event::Help,
        KeyCode::Char('q') => Event::Back,
        KeyCode::Tab => Event::NextScreen,
        KeyCode::Char(c @ '1'..='5') => Event::Navigate(Screen::ALL[(c as u8 - b'1') as usize]),
        KeyCode::Char('p') => Event::SwitchProfile,
        KeyCode::Char('r') => Event::Refresh,
        KeyCode::Char('a') if app.screen == Screen::Overview => Event::Appraise,
        KeyCode::Char('v') => Event::ToggleRawJson,
        KeyCode::Up | KeyCode::Char('k') if app.screen == Screen::Proposals => {
            Event::SelectProposal(-1)
        }
        KeyCode::Down | KeyCode::Char('j') if app.screen == Screen::Proposals => {
            Event::SelectProposal(1)
        }
        KeyCode::Enter if app.screen == Screen::Proposals => Event::SelectProposal(0),
        KeyCode::Left if app.screen == Screen::Run && !app.running => Event::SelectPreset(-1),
        KeyCode::Right if app.screen == Screen::Run && !app.running => Event::SelectPreset(1),
        KeyCode::Char('e') if app.screen == Screen::Run && !app.running => {
            app.wizard.editing = true;
            app.output = None;
            return Vec::new();
        }
        KeyCode::Char('u') if app.screen == Screen::Run && !app.running => {
            app.wizard.upgrade = !app.wizard.upgrade;
            app.wizard.confirmation = None;
            return Vec::new();
        }
        KeyCode::Enter if app.screen == Screen::Run && !app.running => Event::PrepareRun,
        KeyCode::Char('y') if app.screen == Screen::Run => Event::StartRun,
        KeyCode::Enter if app.screen == Screen::Audit => Event::Audit,
        KeyCode::Char('i') if app.screen == Screen::Receipt => {
            app.receipt_editing = true;
            return Vec::new();
        }
        KeyCode::Enter if app.screen == Screen::Receipt => Event::FetchReceipt,
        KeyCode::Char('s') if app.screen == Screen::Receipt => {
            app.receipt_save_editing = true;
            return Vec::new();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll = app.scroll.saturating_add(1);
            app.receipt_scroll = app.receipt_scroll.saturating_add(1);
            return Vec::new();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll = app.scroll.saturating_sub(1);
            app.receipt_scroll = app.receipt_scroll.saturating_sub(1);
            return Vec::new();
        }
        _ => return Vec::new(),
    };
    app::update(app, event)
}
