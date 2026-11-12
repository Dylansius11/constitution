use super::{panel, theme};
use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Paragraph, Wrap},
};
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let areas =
        Layout::vertical([Constraint::Percentage(45), Constraint::Percentage(55)]).split(area);
    let mut rows = Vec::new();
    if let Some(s) = &app.snapshot {
        rows.push(Line::styled(
            format!("Observed at {} · ↑/↓ choose · Enter inspect", s.observed_at),
            theme::MUTED,
        ));
        if s.proposals.is_empty() {
            rows.push(Line::from(
                "No proposals returned in this observation. Check Overview for read errors.",
            ));
        }
        let visible = (areas[0].height.saturating_sub(4) as usize).max(1);
        let start = app
            .selected_proposal
            .saturating_sub(visible.saturating_sub(1));
        for (i, p) in s.proposals.iter().enumerate().skip(start).take(visible) {
            let rejection = if p["rejection"].is_object() {
                format!(
                    " · R{} / {}",
                    p["rejection"]["rule_id"], p["rejection"]["reason_code"]
                )
            } else {
                String::new()
            };
            rows.push(Line::styled(
                format!(
                    "{} · {} · {}{}",
                    p["id"], p["proposer"], p["state"], rejection
                ),
                if i == app.selected_proposal {
                    theme::SELECTED
                } else {
                    theme::BASE
                },
            ));
        }
    } else {
        rows.push(Line::from("Waiting for observed gate proposals…"));
    }
    f.render_widget(Paragraph::new(rows).block(panel("Proposals")), areas[0]);
    let text = if let Some(detail) = &app.detail {
        let mut text = serde_json::to_string_pretty(detail).unwrap_or_else(|e| e.to_string());
        if detail["evm"]["status"] == "included"
            && let Some(hash) = detail["evm"]["tx_hash"].as_str()
        {
            text = format!(
                "Executed Base transaction: https://sepolia.basescan.org/tx/{hash}\n{text}"
            );
        }
        text
    } else if let Some(p) = app
        .snapshot
        .as_ref()
        .and_then(|s| s.proposals.get(app.selected_proposal))
    {
        format!(
            "Gate observation (Enter fetches assembled receipt):\n{}",
            serde_json::to_string_pretty(p).unwrap_or_else(|e| e.to_string())
        )
    } else {
        "Choose an observed proposal to inspect its verdict and evidence root.".into()
    };
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0))
            .block(panel("Detail · verdict / evidence / execution")),
        areas[1],
    );
}
