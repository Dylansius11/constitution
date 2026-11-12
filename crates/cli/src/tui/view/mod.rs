//! Rendering has no I/O and never modifies application state.
mod audit;
mod overview;
mod proposals;
mod receipt;
mod run;
#[cfg(test)]
mod tests;
pub mod theme;
mod ui;

use super::app::{App, Profile, Screen, ToastLevel};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    f.render_widget(ratatui::widgets::Block::default().style(theme::BASE), area);
    let notice = notice_lines(app, area.width);
    let [header, body, notice_area, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(notice.len() as u16),
        Constraint::Length(1),
    ])
    .areas(area);
    render_header(f, header, app);
    match app.screen {
        Screen::Overview => overview::render(f, body, app),
        Screen::Proposals => proposals::render(f, body, app),
        Screen::Run => run::render(f, body, app),
        Screen::Audit => audit::render(f, body, app),
        Screen::Receipt => receipt::render(f, body, app),
    }
    f.render_widget(Paragraph::new(notice), notice_area);
    render_footer(f, footer, app);
    if app.help {
        render_help(f, area);
    } else if app.confirm_cancel {
        render_cancel(f, area);
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let snapshot = app.snapshot.as_ref();
    let (profile_chip, gate_style) = match app.profile {
        Profile::Demo => (
            ui::chip("DEMO · writable", theme::CHIP_ACCENT),
            theme::MUTED,
        ),
        Profile::Final => (
            ui::chip("FINAL · read-only", theme::CHIP_MUTED),
            theme::MUTED,
        ),
    };
    let mut left = vec![
        Span::styled("◆ CONSTITUTION", theme::TITLE),
        Span::raw("  "),
        profile_chip,
    ];
    if let Some(s) = snapshot.filter(|s| !s.gate.is_empty()) {
        left.push(Span::raw("  "));
        left.push(Span::styled(s.gate.clone(), gate_style));
    }
    let mut right = Vec::new();
    match snapshot.and_then(|s| s.access_keys) {
        Some(0) => right.push(ui::chip("● KEYLESS", theme::CHIP_GOOD)),
        Some(n) => right.push(ui::chip(&format!("{n} ACCESS KEYS"), theme::CHIP_BAD)),
        None => {}
    }
    if let Some(s) = snapshot {
        right.push(Span::raw("  "));
        if app.refreshing {
            right.push(Span::styled(
                format!("{} ", theme::spinner(app.now_ns / 1_000_000)),
                theme::ACCENT,
            ));
        }
        right.push(Span::styled(
            format!("observed {}", ui::time_of_day(&s.observed_at)),
            theme::DIM,
        ));
    }
    let [title_row, tab_row, rule_row] = Layout::vertical([Constraint::Length(1); 3]).areas(area);
    let left_width: usize = left.iter().map(|s| ui::cols(&s.content)).sum();
    let right_width: usize = right.iter().map(|s| ui::cols(&s.content)).sum();
    f.render_widget(Paragraph::new(Line::from(left)), title_row);
    if left_width + right_width + 2 <= usize::from(area.width) {
        f.render_widget(Paragraph::new(Line::from(right).right_aligned()), title_row);
    }
    let mut tabs: Vec<Span<'static>> = Vec::new();
    for (i, screen) in Screen::ALL.iter().enumerate() {
        if *screen == app.screen {
            tabs.push(Span::styled(
                format!(" {} {} ", i + 1, screen.title()),
                theme::TAB_ACTIVE,
            ));
        } else {
            tabs.push(Span::styled(format!(" {} ", i + 1), theme::DIM));
            tabs.push(Span::styled(format!("{} ", screen.title()), theme::MUTED));
        }
        tabs.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(tabs)), tab_row);
    f.render_widget(
        Paragraph::new(ui::rule(usize::from(area.width), theme::BORDER)),
        rule_row,
    );
}

/// Error and toast lines under the body; empty when there is nothing to say.
fn notice_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let room = usize::from(width).saturating_sub(3).max(10);
    if let Some(error) = &app.error {
        return ui::wrap(error, room)
            .into_iter()
            .take(3)
            .map(|line| {
                Line::from(vec![
                    Span::styled("▌ ", theme::BAD),
                    Span::styled(line, theme::WARN),
                ])
            })
            .collect();
    }
    if let Some(result) = &app.last_action_result {
        let (bar, style) = match result.level {
            ToastLevel::Info => (theme::ACCENT, theme::MUTED),
            ToastLevel::Success => (theme::GOOD, theme::GOOD),
            ToastLevel::Error => (theme::BAD, theme::BAD),
        };
        let stamp = result
            .observed_at
            .as_deref()
            .map_or(String::new(), |t| format!("  {}", ui::time_of_day(t)));
        let message = ui::clip(&result.message, room.saturating_sub(ui::cols(&stamp)));
        return vec![Line::from(vec![
            Span::styled("▌ ", bar),
            Span::styled(message, style),
            Span::styled(stamp, theme::DIM),
        ])];
    }
    Vec::new()
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let mut hints: Vec<(&str, &str)> = match app.screen {
        Screen::Overview => vec![
            ("a", "appraise"),
            ("r", "refresh"),
            ("v", "raw"),
            ("p", "profile"),
        ],
        Screen::Proposals => vec![
            ("↑↓", "select"),
            ("Enter", "detail"),
            ("v", "raw JSON"),
            ("r", "refresh"),
        ],
        Screen::Run => {
            if app.wizard.editing {
                vec![
                    ("Tab", "next field"),
                    ("Enter", "done"),
                    ("Esc", "stop editing"),
                ]
            } else if app.running {
                vec![("Ctrl+C", "cancel")]
            } else if app.wizard.confirmation.is_some() {
                vec![("y", "authorize"), ("Esc", "back"), ("j/k", "scroll")]
            } else {
                vec![
                    ("←→", "scenario"),
                    ("e", "edit"),
                    ("u", "attack/upgrade"),
                    ("Enter", "review"),
                ]
            }
        }
        Screen::Audit => vec![("Enter", "run audit"), ("j/k", "scroll")],
        Screen::Receipt => {
            if app.receipt_editing || app.receipt_save_editing {
                vec![("Enter", "confirm"), ("Esc", "cancel")]
            } else {
                vec![
                    ("i", "proposal id"),
                    ("Enter", "fetch"),
                    ("s", "save"),
                    ("j/k", "scroll"),
                ]
            }
        }
    };
    hints.extend([("Tab", "screens"), ("?", "help"), ("q", "back")]);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for (key, label) in hints {
        let width = ui::cols(key) + ui::cols(label) + 6;
        if used + width > usize::from(area.width) {
            break;
        }
        used += width;
        spans.extend(ui::keycap(key, label));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

const HELP: [(&str, &[(&str, &str)]); 6] = [
    (
        "Anywhere",
        &[
            ("1–5", "jump to a screen"),
            ("Tab", "next screen"),
            ("p", "Demo / Final profile"),
            ("r", "refresh (also every 30 s)"),
            ("v", "show or hide raw JSON"),
            ("q  Esc", "close, back, then quit"),
            ("Ctrl+C", "cancel a running write"),
        ],
    ),
    (
        "Overview",
        &[("a", "appraise the demo verifier"), ("j/k", "scroll")],
    ),
    (
        "Proposals",
        &[
            ("↑↓  j/k", "select a proposal"),
            ("Enter", "fetch its detail"),
        ],
    ),
    (
        "Run",
        &[
            ("←→", "choose a scenario"),
            ("e", "edit the inputs"),
            ("Tab", "next field while editing"),
            ("u", "toggle attack / upgrade"),
            ("Enter", "review; nothing is written yet"),
            ("y", "authorize the reviewed write"),
        ],
    ),
    ("Audit", &[("Enter", "run the read-only audit")]),
    (
        "Receipt",
        &[
            ("i", "set the proposal id"),
            ("Enter", "fetch the receipt"),
            ("s", "save it (create-only)"),
        ],
    ),
];

fn render_help(f: &mut Frame, area: Rect) {
    let popup = centered(area, 100, 24);
    let section = |name: &str, keys: &[(&str, &str)]| -> Vec<Line<'static>> {
        let mut lines = vec![Line::styled(name.to_uppercase(), theme::TITLE)];
        for (key, label) in keys {
            lines.push(Line::from(vec![
                Span::styled(format!(" {key:<7}"), theme::KEYCAP),
                Span::styled(format!(" {label}"), theme::MUTED),
            ]));
        }
        lines.push(Line::default());
        lines
    };
    f.render_widget(Clear, popup);
    let block = ui::card("Keys", theme::BORDER_FOCUS);
    let body = block.inner(popup);
    f.render_widget(block, popup);
    if popup.width >= 80 {
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(body);
        let first: Vec<_> = HELP[..3].iter().flat_map(|(n, k)| section(n, k)).collect();
        let second: Vec<_> = HELP[3..].iter().flat_map(|(n, k)| section(n, k)).collect();
        f.render_widget(Paragraph::new(first), left);
        f.render_widget(Paragraph::new(second), right);
    } else {
        let all: Vec<_> = HELP.iter().flat_map(|(n, k)| section(n, k)).collect();
        f.render_widget(Paragraph::new(all), body);
    }
}

fn render_cancel(f: &mut Frame, area: Rect) {
    let popup = centered(area, 78, 16);
    let inner = ui::inner_width(popup.width);
    let mut lines = vec![Line::styled(
        "A NEAR write may already have been sent.",
        theme::WARN,
    )];
    lines.push(Line::default());
    for paragraph in [
        "This confirmation is conservative from process launch, including while an appraisal is still running. Stopping the local process cannot undo credit funding, a registered proposal, verifier work, an MPC request or a Base transaction that was already broadcast. The remote verifier may keep working after you cancel.",
        "Inspect submission.json, the run status and the receipt before any retry. Never create a duplicate proposal to recover.",
    ] {
        lines.extend(
            ui::wrap(paragraph, inner)
                .into_iter()
                .map(|l| Line::styled(l, theme::BASE)),
        );
        lines.push(Line::default());
    }
    lines.push(Line::from(
        [
            ui::keycap("y", "stop the local worker"),
            ui::keycap("n", "keep watching"),
        ]
        .concat(),
    ));
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(lines).block(ui::card("Stop the run?", theme::BORDER_WARN)),
        popup,
    );
}
