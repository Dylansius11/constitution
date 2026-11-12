use super::{panel, theme};
use crate::tui::app::{App, Availability, Profile, StepState};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    if app.profile == Profile::Final {
        f.render_widget(Paragraph::new("Final gate is read-only. Its verifier is retired; use recorded evidence via Proposals / Receipt / Audit.\n\nPress p to choose Demo gate for interactive writes.").style(theme::WARN).wrap(Wrap{trim:false}).block(panel("Run disabled")),area);
        return;
    }
    if let Some(confirmation) = &app.wizard.confirmation {
        f.render_widget(
            Paragraph::new(confirmation.as_str())
                .wrap(Wrap { trim: false })
                .scroll((app.scroll, 0))
                .block(panel(
                    "Confirm exact observed inputs and spending · y signs / Esc back",
                )),
            area,
        );
        return;
    }
    if app.running || app.output.is_some() {
        let areas = Layout::vertical([Constraint::Length(5), Constraint::Min(4)]).split(area);
        let spans: Vec<Span<'_>> = app
            .steps
            .iter()
            .flat_map(|step| {
                let (label, style) = match step.state {
                    StepState::Pending => ("pending", theme::MUTED),
                    StepState::Active => ("active", theme::ACTIVE),
                    StepState::Pass => ("pass", theme::GOOD),
                    StepState::Refused => ("refused", theme::WARN),
                    StepState::Error => ("error", theme::BAD),
                    StepState::NotRun => ("not-run", theme::MUTED),
                };
                [
                    Span::styled(
                        format!(
                            "{} [{} {:.1}s]",
                            step.name,
                            label,
                            step.elapsed_ms as f64 / 1000.0
                        ),
                        style,
                    ),
                    Span::raw(" → "),
                ]
            })
            .collect();
        f.render_widget(
            Paragraph::new(Line::from(spans))
                .wrap(Wrap { trim: false })
                .block(panel(&format!(
                    "{} · total {:.1}s · Ctrl+C cancels",
                    if app.cancelling {
                        "Cancellation requested · waiting for observation worker"
                    } else if app.running {
                        "Live run"
                    } else {
                        "Run stopped"
                    },
                    app.elapsed_ms as f64 / 1000.0
                ))),
            areas[0],
        );
        let mut lines = vec![
            Line::from(format!(
                "Artifacts: {}",
                app.output.as_deref().unwrap_or("reserving directory")
            )),
            Line::styled(
                "Stage durations are verifier-reported when available; active timings are observed locally, not backfilled.",
                theme::MUTED,
            ),
        ];
        lines.push(Line::from(format!(
            "Run observed start: {} · finish: {}",
            app.run_started_at.as_deref().unwrap_or("not observed"),
            app.run_finished_at.as_deref().unwrap_or("not observed")
        )));
        for s in &app.steps {
            if !s.detail.is_empty() {
                lines.push(Line::from(format!(
                    "{} · observed start {} / finish {} · {}",
                    s.name,
                    s.started_at.as_deref().unwrap_or("not observed"),
                    s.finished_at.as_deref().unwrap_or("not observed"),
                    s.detail
                )));
            }
        }
        lines.push(Line::styled("Observed CLI log", theme::TITLE));
        lines.extend(
            app.run_log
                .iter()
                .rev()
                .take(20)
                .rev()
                .map(|s| Line::from(s.as_str())),
        );
        if !app.running {
            lines.push(Line::from("e: edit inputs / ← or → choose preset / Enter prepare a NEW run; inspect recovery artifacts first."));
        }
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((app.scroll, 0))
                .block(panel("Real stages / errors and recovery")),
            areas[1],
        );
        return;
    }
    let current = app
        .snapshot
        .as_ref()
        .and_then(|s| s.implementation.as_deref());
    let mut lines = vec![
        Line::from(
            "1 Choose signer  →  2 Preset or custom  →  3 Review spending  →  4 Explicit approval",
        ),
        Line::styled(
            format!(
                "Current proxy implementation: {}",
                current.unwrap_or("not yet observed")
            ),
            theme::MUTED,
        ),
    ];
    if let Some(p) = app.presets.get(app.wizard.preset) {
        let available = match p.availability(current) {
            Availability::Ready => "available",
            Availability::Done => "DONE / unavailable",
            Availability::RequiresPrevious => "unavailable: apply V2 first",
            Availability::Unknown => "unavailable until current implementation is observed",
        };
        lines.push(Line::styled(
            format!(
                "Preset ←/→: {} ({}) · {} · {}",
                p.title,
                p.id,
                p.rule.map_or_else(
                    || "honest upgrade (all rules)".into(),
                    |r| format!("targets R{r}")
                ),
                available
            ),
            theme::TITLE,
        ));
        if let Some(previous) = &p.previous_run {
            lines.push(Line::from(format!(
                "Previous recorded run took {:.3} s · observed {} (not an estimate)",
                previous.elapsed_ms as f64 / 1000.0,
                previous
                    .observed_at
                    .as_deref()
                    .unwrap_or("capture time not recorded")
            )));
        }
    }
    lines.push(Line::from(format!(
        "Mode: {} (u toggles for custom bundle)",
        if app.wizard.upgrade {
            "upgrade — relay only after Signed"
        } else {
            "attack — never relay"
        }
    )));
    for (i, (label, value)) in crate::tui::app::Wizard::LABELS
        .iter()
        .zip(app.wizard.fields())
        .enumerate()
    {
        lines.push(Line::styled(
            format!(
                "{} {label}: {value}",
                if app.wizard.field == i { ">" } else { " " }
            ),
            if app.wizard.editing && app.wizard.field == i {
                theme::SELECTED
            } else {
                theme::BASE
            },
        ));
    }
    lines.push(Line::from(if app.preparing {
        "Preparing confirmation: validating path/source and reading live credit, fees and proxy state…"
    } else if app.wizard.editing {
        "Editing: type or paste · Tab/Shift+Tab fields · Enter finish · Esc back"
    } else {
        "e edits signer/custom fields · Enter validates + reads live funding confirmation"
    }));
    lines.push(Line::styled(
        "No signing occurs until the confirmation screen's y. Key-file contents are never shown.",
        theme::WARN,
    ));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0))
            .block(panel("Run · guided wizard")),
        area,
    );
}
