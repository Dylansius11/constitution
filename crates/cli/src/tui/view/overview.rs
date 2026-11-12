use super::{panel, theme};
use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Paragraph, Wrap},
};
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(s) = &app.snapshot else {
        f.render_widget(Paragraph::new("Reading public state… no live values observed yet.\nr: refresh; configure NEAR_RPC_URL / BASE_SEPOLIA_RPC_URL if unavailable.").block(panel("Overview")),area);
        return;
    };
    let mut lines = vec![
        Line::styled(
            format!(
                "Observed at {}{}",
                s.observed_at,
                if app.refreshing { " · refreshing" } else { "" }
            ),
            theme::MUTED,
        ),
        Line::from(format!("Gate: {}", s.gate)),
        Line::from(format!("Gate explorer: {}", s.gate_url)),
        Line::from(format!("Proxy explorer: {}", s.proxy_url)),
        Line::from(match s.access_keys {
            Some(0) => "Access keys: 0 — KEYLESS".into(),
            Some(n) => format!("Access keys: {n} — NOT KEYLESS"),
            None => "Access keys: unavailable".into(),
        }),
        Line::from(format!(
            "Active rule-set: {}",
            s.rule_hash.as_deref().unwrap_or("unavailable")
        )),
        Line::from(format!(
            "Local committed manifest: {}",
            s.manifest_hash.as_deref().unwrap_or("unavailable")
        )),
    ];
    lines.push(Line::styled(
        match (&s.rule_hash, &s.manifest_hash) {
            (Some(a), Some(b)) if a == b => "Manifest binding: MATCH",
            (Some(_), Some(_)) => "Manifest binding: MISMATCH",
            _ => "Manifest binding: UNKNOWN",
        },
        if s.rule_hash.is_some() && s.rule_hash == s.manifest_hash {
            theme::GOOD
        } else {
            theme::WARN
        },
    ));
    lines.extend([
        Line::from(format!(
            "Base Sepolia proxy implementation: {}",
            s.implementation.as_deref().unwrap_or("unavailable")
        )),
        Line::from(format!(
            "Governance delay: {} (ordinary upgrades do not wait)",
            s.delay_days
                .map_or_else(|| "unavailable".into(), |d| format!("{d} day(s)"))
        )),
        Line::from(format!("Verifier: {}", s.verifier)),
    ]);
    if let Some(identity) = &s.identity {
        lines.push(Line::styled(
            format!(
                "Identity source: {} · {}",
                if identity.recorded {
                    "RECORDED (retired)"
                } else {
                    "live endpoint; not an appraisal"
                },
                identity
                    .observed_at
                    .as_deref()
                    .unwrap_or("capture time not recorded")
            ),
            theme::MUTED,
        ));
        for (value, label) in [
            (&identity.verifier_pubkey, "Certificate public key"),
            (&identity.release_hash, "Release hash"),
            (&identity.compose_hash, "Compose hash"),
        ] {
            lines.push(Line::from(format!(
                "{label}: {}",
                value.as_deref().unwrap_or("unavailable")
            )));
        }
    }
    if app.appraising {
        lines.push(Line::styled(
            "Appraising pinned demo identity… no pass assumed",
            theme::ACTIVE,
        ));
    } else {
        lines.push(Line::from(
            "a: explicitly appraise Demo verifier (Final never probes)",
        ));
    }
    if let Some(result) = &app.appraisal {
        let label = if result.recorded {
            "RECORDED appraisal"
        } else if result.ok() {
            "Appraised"
        } else {
            "APPRAISAL FAILED"
        };
        lines.push(Line::styled(
            format!(
                "{label} {}/{} · {} · recorded date {}",
                result.passed(),
                result.checks.len(),
                result
                    .observed_at
                    .as_deref()
                    .unwrap_or("appraisal capture time not recorded"),
                result.recorded_date.as_deref().unwrap_or("live")
            ),
            if result.ok() { theme::GOOD } else { theme::BAD },
        ));
        for check in &result.checks {
            lines.push(Line::from(format!(
                "{} {}: {}",
                if check.ok { "PASS" } else { "FAIL" },
                check.name,
                check.detail
            )));
        }
    }
    lines.push(Line::styled(
        format!("Timelock queue: {} observed entries", s.changes.len()),
        theme::TITLE,
    ));
    for change in &s.changes {
        if app.show_raw_json {
            lines.push(Line::from(change.raw.to_string()));
        } else {
            lines.push(Line::from(format!("Change {} · {} · {} · ETA {} · {} seconds remaining{} (local-clock countdown, not execution authorization)",change.id,change.kind,change.status,change.eta_utc,change.remaining_seconds,if change.matured {" · ETA reached"}else{""})));
        }
    }
    for error in &s.errors {
        lines.push(Line::styled(
            format!("READ ERROR: {error}; r to retry after checking RPC configuration"),
            theme::BAD,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0))
            .block(panel(
                "Overview · live reads, finality: NEAR final / EVM latest",
            )),
        area,
    );
}
