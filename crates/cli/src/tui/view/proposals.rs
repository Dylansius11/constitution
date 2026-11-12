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
            format!(
                "Observed at {} · ↑/↓ choose · Enter inspect · v raw JSON",
                s.observed_at
            ),
            theme::MUTED,
        ));
        if s.proposals.is_empty() {
            rows.push(Line::from(
                "No proposals returned; check Overview for read errors.",
            ));
        }
        let visible = (areas[0].height.saturating_sub(4) as usize).max(1);
        let start = app
            .selected_proposal
            .saturating_sub(visible.saturating_sub(1));
        for (i, p) in s.proposals.iter().enumerate().skip(start).take(visible) {
            let refusal = p.refusal.as_ref().map_or_else(String::new, |r| {
                format!(" · R{} / {}", r.rule_id, r.reason_code)
            });
            rows.push(Line::styled(
                format!("{} · {} · {:?}{}", p.id, p.proposer, p.state, refusal),
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
    let selected = app
        .detail
        .as_ref()
        .or_else(|| app.snapshot.as_ref()?.proposals.get(app.selected_proposal));
    let Some(p) = selected else {
        f.render_widget(
            Paragraph::new("Choose an observed proposal.").block(panel("Detail")),
            areas[1],
        );
        return;
    };
    if app.show_raw_json {
        let text = serde_json::to_string_pretty(&p.raw).unwrap_or_else(|e| e.to_string());
        f.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((app.scroll, 0))
                .block(panel("Raw observed JSON · v returns to fields")),
            areas[1],
        );
        return;
    }
    let mut lines = vec![
        Line::from(format!(
            "Proposal {} · {} · {:?}",
            p.id, p.proposer, p.state
        )),
        Line::from(format!(
            "Created: {} · Expires: {}",
            p.created_at, p.expires_at
        )),
    ];
    if let Some(r) = &p.refusal {
        lines.push(Line::styled(
            format!("R{} / {} — {}", r.rule_id, r.reason_code, r.meaning),
            theme::WARN,
        ));
    }
    for (label, value) in [
        ("New implementation", p.new_impl.as_str()),
        ("Runtime codehash", p.codehash.as_str()),
        (
            "Expected current implementation",
            p.expected_current_impl.as_str(),
        ),
        ("Init calldata", p.init_calldata.as_str()),
        ("Source commitment", p.source_commitment.as_str()),
        (
            "Certificate hash",
            p.certificate_hash.as_deref().unwrap_or("not observed"),
        ),
        (
            "Evidence root",
            p.evidence_root.as_deref().unwrap_or("not observed"),
        ),
        (
            "EVM transaction hash",
            p.evm_tx_hash.as_deref().unwrap_or("not observed"),
        ),
        (
            "EVM state",
            p.evm_status
                .as_deref()
                .unwrap_or("not observed by an EVM receipt read"),
        ),
    ] {
        lines.push(Line::from(format!("{label}: {value}")));
    }
    lines.push(Line::from(format!("Gate: {}", p.gate_url)));
    lines.push(Line::from(format!(
        "New implementation: {}",
        p.new_impl_url
    )));
    lines.push(Line::from(format!(
        "Expected current: {}",
        p.current_impl_url
    )));
    if let Some(url) = &p.evm_tx_url {
        lines.push(Line::from(format!(
            "Base transaction (inclusion only when state=included): {url}"
        )));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0))
            .block(panel(
                "Typed detail · verdict / evidence / execution · v raw",
            )),
        areas[1],
    );
}
