//! Rendering has no I/O and never modifies application state.
mod audit;
mod overview;
mod proposals;
mod receipt;
mod run;
pub mod theme;
use super::app::{App, Screen};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
pub fn panel(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .title_style(theme::TITLE)
        .borders(Borders::ALL)
        .border_style(theme::BORDER)
}
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    f.render_widget(Block::default().style(theme::BASE), area);
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(if app.error.is_some() { 3 } else { 1 }),
        Constraint::Length(1),
    ])
    .split(area);
    f.render_widget(
        Paragraph::new(format!(
            "CONSTITUTION  |  {}  [p: switch profile]",
            app.profile.label()
        ))
        .style(theme::TITLE),
        chunks[0],
    );
    let tabs: Vec<Span<'_>> = Screen::ALL
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            [
                Span::styled(
                    format!(" {} {} ", i + 1, s.title()),
                    if *s == app.screen {
                        theme::SELECTED
                    } else {
                        theme::MUTED
                    },
                ),
                Span::raw(" "),
            ]
        })
        .collect();
    f.render_widget(Paragraph::new(Line::from(tabs)), chunks[1]);
    match app.screen {
        Screen::Overview => overview::render(f, chunks[2], app),
        Screen::Proposals => proposals::render(f, chunks[2], app),
        Screen::Run => run::render(f, chunks[2], app),
        Screen::Audit => audit::render(f, chunks[2], app),
        Screen::Receipt => receipt::render(f, chunks[2], app),
    }
    if let Some(error) = &app.error {
        f.render_widget(
            Paragraph::new(error.as_str())
                .style(theme::WARN)
                .wrap(Wrap { trim: false }),
            chunks[3],
        );
    }
    f.render_widget(
        Paragraph::new(
            "? help · Tab / 1–5 screens · p profile · r refresh · q / Esc back · Ctrl+C cancel",
        )
        .style(theme::MUTED),
        chunks[4],
    );
    if app.help || app.confirm_cancel {
        let width = area.width.saturating_sub(4).min(100);
        let height = area.height.saturating_sub(4).min(22);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let text = if app.confirm_cancel {
            "A NEAR write MAY already have been sent. This confirmation is conservative from process launch, including while appraisal is still in progress.\n\nStopping the local process cannot undo credit funding, a registered proposal, verifier work, an MPC request, or a Base transaction already broadcast. The remote verifier may continue after cancellation.\n\nInspect submission.json, status and receipt before any retry. Never create a duplicate proposal to recover.\n\ny: stop local worker / n or Esc: keep watching"
        } else {
            "GLOBAL\nTab or 1–5: switch screens • p: Demo / Final profile\nr: refresh observed public state (also every 30s)\nq / Esc: close overlay, leave editor, back to Overview, then quit\nCtrl+C: cancel a run; confirmation after a possible NEAR write\nj/k or arrows: scroll (Proposals: choose row)\n\nRUN\n←/→: presets • e: edit fields • u: attack / upgrade (custom)\nEditor: Tab/Shift+Tab next/previous field; Enter finish editing\nEnter: validate paths + read funding/signing confirmation\ny: confirm signing/spending (only on confirmation screen)\n\nAUDIT: Enter runs read-only checks\nPROPOSALS: Enter reads selected receipt detail\nRECEIPT: i edits proposal id; Enter fetches; s edits new JSON path\nEnter in a receipt input submits; existing files are not overwritten\n\nFinal profile is evidence-only: all write actions disabled."
        };
        f.render_widget(Clear, popup);
        f.render_widget(
            Paragraph::new(text)
                .style(theme::BASE)
                .wrap(Wrap { trim: false })
                .block(panel(if app.confirm_cancel {
                    "Confirm cancellation"
                } else {
                    "Keyboard help"
                })),
            popup,
        );
    }
}
