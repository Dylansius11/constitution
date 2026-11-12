use super::{theme, ui};
use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::Paragraph,
};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let [summary, list] = Layout::vertical([Constraint::Length(3), Constraint::Min(4)]).areas(area);
    let width = ui::inner_width(area.width);

    let mut head = Vec::new();
    if app.auditing {
        head.push(ui::text(
            format!(
                "{} Reading audit preconditions…  ",
                theme::spinner(app.now_ns / 1_000_000)
            ),
            theme::ACCENT,
        ));
    }
    match &app.audit {
        Some(audit) => {
            let warnings = audit.checks.iter().filter(|c| c.warning).count();
            let failed = audit.checks.iter().filter(|c| !c.ok && !c.warning).count();
            let passed = audit.checks.len() - warnings - failed;
            head.push(if audit.exit == 0 {
                ui::chip(&format!("PASS · exit {}", audit.exit), theme::CHIP_GOOD)
            } else {
                ui::chip(&format!("FAIL · exit {}", audit.exit), theme::CHIP_BAD)
            });
            head.push(ui::text(
                format!(
                    "  {passed} passed · {warnings} warnings · {failed} failed · observed {}",
                    ui::time_of_day(&audit.observed_at)
                ),
                theme::MUTED,
            ));
        }
        None if !app.auditing => {
            head.extend(ui::keycap(
                "Enter",
                "run the read-only audit for this profile",
            ));
        }
        None => {}
    }
    f.render_widget(
        Paragraph::new(Line::from(head)).block(ui::card("Audit", theme::BORDER_FOCUS)),
        summary,
    );

    let block = ui::card("Checks", theme::BORDER);
    let inner = block.inner(list);
    f.render_widget(block, list);
    let Some(audit) = &app.audit else {
        f.render_widget(
            Paragraph::new(vec![
                Line::styled("No audit result observed yet.", theme::MUTED),
                Line::styled(
                    "The audit needs the committed executor Foundry artifact; an error is never a pass.",
                    theme::DIM,
                ),
            ]),
            inner,
        );
        return;
    };
    let name = audit
        .checks
        .iter()
        .map(|c| ui::cols(&c.name))
        .max()
        .unwrap_or(0);
    let detail_width = width.saturating_sub(name + 5).max(10);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for check in &audit.checks {
        let (icon, style) = if check.warning {
            ("!", theme::WARN)
        } else if check.ok {
            (theme::PASS, theme::GOOD)
        } else {
            (theme::FAIL, theme::BAD)
        };
        // Passing checks stay on one line; anything that needs attention shows its whole reason.
        let pieces = if check.ok && !check.warning {
            vec![ui::clip(&check.detail, detail_width)]
        } else {
            ui::wrap(&check.detail, detail_width)
        };
        for (n, piece) in pieces.into_iter().enumerate() {
            if n == 0 {
                lines.push(Line::from(vec![
                    ui::text(format!("{icon} "), style),
                    ui::text(
                        format!("{:<name$}  ", check.name),
                        if check.ok && !check.warning {
                            theme::BASE
                        } else {
                            style
                        },
                    ),
                    ui::text(piece, theme::DIM),
                ]));
            } else {
                lines.push(Line::styled(
                    format!("{}{piece}", " ".repeat(name + 4)),
                    theme::DIM,
                ));
            }
        }
    }
    let height = usize::from(inner.height);
    let (visible, offset) = ui::window(&lines, height, app.scroll);
    f.render_widget(Paragraph::new(visible), inner);
    ui::scrollbar(f, inner, lines.len(), height, offset);
}
