use super::{theme, ui};
use crate::tui::app::{App, ProposalView};
use constitution_types::ProposalState;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

const LABEL: usize = 14;

/// Badge text, chip style and the plain-English meaning of each gate proposal state.
fn describe(state: &ProposalState) -> (&'static str, Style, &'static str) {
    match *state {
        ProposalState::Rejected => (
            "REFUSED",
            theme::CHIP_ACCENT,
            "The verifier refused this upgrade and the gate recorded it. Nothing was signed and the proxy is unchanged.",
        ),
        ProposalState::Signed => (
            "SIGNED",
            theme::CHIP_GOOD,
            "Every rule passed and the gate obtained a signature for this exact upgrade.",
        ),
        ProposalState::Pending => (
            "PENDING",
            theme::CHIP_WARN,
            "Waiting for a verifier certificate. The proposal expires if none arrives in time.",
        ),
        ProposalState::SignRequested => (
            "SIGNING",
            theme::CHIP_WARN,
            "Certified. The gate asked Chain Signatures for a signature that has not been observed yet.",
        ),
        ProposalState::SignFailed => (
            "SIGN FAILED",
            theme::CHIP_BAD,
            "The signature request failed. Nothing reached Base; inspect the raw record before retrying.",
        ),
        ProposalState::Expired => (
            "EXPIRED",
            theme::CHIP_MUTED,
            "No certificate arrived before the deadline. Nothing was signed.",
        ),
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(s) = &app.snapshot else {
        f.render_widget(
            Paragraph::new(Line::styled(
                format!(
                    "{} Waiting for the gate's proposals…",
                    theme::spinner(app.now_ns / 1_000_000)
                ),
                theme::ACCENT,
            ))
            .block(ui::card("Proposals", theme::BORDER)),
            area,
        );
        return;
    };
    let [list, detail] = if area.width >= 104 {
        Layout::horizontal([Constraint::Length(50), Constraint::Min(40)]).areas(area)
    } else {
        Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(area)
    };
    render_list(f, list, app, s);
    let selected = app
        .detail
        .as_ref()
        .or_else(|| s.proposals.get(app.selected_proposal));
    match selected {
        Some(p) => render_detail(f, detail, app, p),
        None => f.render_widget(
            Paragraph::new(Line::styled("Choose a proposal on the left.", theme::MUTED))
                .block(ui::card("Detail", theme::BORDER)),
            detail,
        ),
    }
}

fn render_list(f: &mut Frame, area: Rect, app: &App, s: &crate::tui::app::Snapshot) {
    let block = ui::card(
        &format!("Proposals · {}", s.proposals.len()),
        theme::BORDER_FOCUS,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    if s.proposals.is_empty() {
        f.render_widget(
            Paragraph::new(vec![
                Line::styled("No proposals returned.", theme::MUTED),
                Line::styled("Check Overview for read errors, then press r.", theme::DIM),
            ]),
            inner,
        );
        return;
    }
    let width = usize::from(inner.width);
    let [header, rows] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    f.render_widget(
        Paragraph::new(Line::styled(
            format!("  {:<5}{:<13}{:<8}{}", "ID", "STATE", "RULE", "PROPOSER"),
            theme::DIM,
        )),
        header,
    );
    let visible = usize::from(rows.height).max(1);
    let selected = app.selected_proposal.min(s.proposals.len() - 1);
    let start = (selected + 1).saturating_sub(visible);
    let proposer_width = width.saturating_sub(2 + 5 + 13 + 8);
    let lines: Vec<Line<'static>> = s
        .proposals
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(i, p)| {
            let (label, chip, _) = describe(&p.state);
            let rule = p.refusal.as_ref().map_or(String::new(), |r| {
                format!("R{}/{}", r.rule_id, r.reason_code)
            });
            let marker = if i == selected {
                Span::styled(format!("{} ", theme::SELECTOR), theme::TITLE)
            } else {
                Span::raw("  ")
            };
            let mut line = Line::from(vec![
                marker,
                ui::text(format!("{:<5}", format!("#{}", p.id)), theme::STRONG),
                Span::styled(format!(" {label} "), chip),
                ui::text(
                    " ".repeat(13usize.saturating_sub(ui::cols(label) + 2)),
                    theme::BASE,
                ),
                ui::text(format!("{rule:<8}"), theme::ACCENT),
                ui::text(ui::clip(&p.proposer, proposer_width), theme::MUTED),
            ]);
            if i == selected {
                line = line.style(theme::SELECTED_ROW);
            }
            line
        })
        .collect();
    f.render_widget(Paragraph::new(lines), rows);
    ui::scrollbar(f, rows, s.proposals.len(), visible, start);
}

fn render_detail(f: &mut Frame, area: Rect, app: &App, p: &ProposalView) {
    let (label, chip, meaning) = describe(&p.state);
    let block = ui::card(&format!("Proposal #{}", p.id), theme::BORDER);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let width = usize::from(inner.width);
    let mut lines: Vec<Line<'static>> = Vec::new();

    if app.show_raw_json {
        lines.push(Line::from(vec![
            ui::chip("RAW JSON", theme::CHIP_MUTED),
            ui::text("  v returns to the typed fields", theme::DIM),
        ]));
        lines.push(Line::default());
        let pretty = serde_json::to_string_pretty(&p.raw).unwrap_or_else(|e| e.to_string());
        lines.extend(ui::json_lines(&pretty, width));
        let height = usize::from(inner.height);
        if lines.len() > height {
            let hidden = lines.len() - height + 1;
            lines.truncate(height - 1);
            lines.push(Line::styled(
                format!("… {hidden} more lines; the full record is in the receipt"),
                theme::DIM,
            ));
        }
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let mut headline = vec![Span::styled(format!(" {label} "), chip)];
    if let Some(r) = &p.refusal {
        headline.push(ui::text(
            format!("  Rule {} · reason {}", r.rule_id, r.reason_code),
            theme::TITLE,
        ));
    }
    lines.push(Line::from(headline));
    lines.push(Line::default());
    let mut explanation = String::from(meaning);
    if let Some(r) = &p.refusal {
        explanation = format!("{}. {}", r.meaning.trim_end_matches('.'), meaning);
    }
    lines.extend(
        ui::wrap(&explanation, width)
            .into_iter()
            .map(|l| Line::styled(l, theme::BASE)),
    );
    lines.push(Line::default());

    let value_width = width.saturating_sub(LABEL);
    let field = |name: &str, value: &str, style: Style| {
        ui::kv(
            name,
            LABEL,
            vec![ui::text(ui::fit(value, value_width), style)],
        )
    };
    lines.push(field("Proposer", &p.proposer, theme::BASE));
    lines.push(field("Created", &p.created_at, theme::BASE));
    lines.push(field("Expires", &p.expires_at, theme::BASE));
    lines.push(Line::default());
    lines.push(field("New impl", &p.new_impl, theme::BASE));
    lines.push(field("Codehash", &p.codehash, theme::MUTED));
    lines.push(field(
        "Expected impl",
        &p.expected_current_impl,
        theme::MUTED,
    ));
    lines.push(field("Init calldata", &p.init_calldata, theme::MUTED));
    lines.push(field("Source commit", &p.source_commitment, theme::MUTED));
    lines.push(Line::default());
    let observed =
        |v: &Option<String>, missing: &str| v.clone().unwrap_or_else(|| missing.to_owned());
    lines.push(field(
        "Certificate",
        &observed(&p.certificate_hash, "not observed"),
        theme::MUTED,
    ));
    lines.push(field(
        "Evidence root",
        &observed(&p.evidence_root, "not observed"),
        theme::MUTED,
    ));
    lines.push(field(
        "Base tx",
        &observed(&p.evm_tx_hash, "none · nothing was relayed"),
        theme::MUTED,
    ));
    if let Some(status) = &p.evm_status {
        lines.push(field("Base state", status, theme::GOOD));
    }
    lines.push(Line::default());
    lines.push(field("Gate", &p.gate_url, theme::DIM));
    lines.push(field("New impl", &p.new_impl_url, theme::DIM));
    lines.push(field("Expected impl", &p.current_impl_url, theme::DIM));
    if let Some(url) = &p.evm_tx_url {
        lines.push(field("Base tx", url, theme::DIM));
    }
    let height = usize::from(inner.height);
    let mut lines = ui::compact(lines, height);
    if lines.len() > height {
        let hidden = lines.len() - height + 1;
        lines.truncate(height - 1);
        lines.push(Line::styled(
            format!("… {hidden} more lines; press v for the raw record"),
            theme::DIM,
        ));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
