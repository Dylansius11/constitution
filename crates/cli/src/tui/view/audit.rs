use super::{panel, theme};
use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Paragraph, Wrap},
};
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![Line::from(if app.auditing {
        "Reading audit preconditions… UI remains interactive."
    } else {
        "Enter: run read-only audit for active profile · j/k scroll"
    })];
    if let Some(audit) = &app.audit {
        lines.push(Line::styled(
            format!("Observed at {} · exit {}", audit.observed_at, audit.exit),
            if audit.exit == 0 {
                theme::GOOD
            } else {
                theme::BAD
            },
        ));
        for c in &audit.checks {
            lines.push(Line::styled(
                format!(
                    "{} {}: {}",
                    if c.warning {
                        "WARN"
                    } else if c.ok {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                    c.name,
                    c.detail
                ),
                if c.warning {
                    theme::WARN
                } else if c.ok {
                    theme::GOOD
                } else {
                    theme::BAD
                },
            ));
        }
    } else {
        lines.push(Line::from("No audit result observed. Requires committed executor Foundry artifact; errors are not a pass."));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0))
            .block(panel("Audit · unchanged F7 checks")),
        area,
    );
}
