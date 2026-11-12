use super::{theme, ui};
use crate::tui::app::{App, Availability, Preset, Profile, STAGES, Step, StepState};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

const FORM_LABELS: [&str; 6] = [
    "NEAR account",
    "Key file",
    "Bundle dir",
    "Contract",
    "Implementation",
    "Init calldata",
];
const FORM_LABEL: usize = 16;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    if app.profile == Profile::Final {
        let lines = vec![
            Line::styled("The final gate is read-only.", theme::WARN),
            Line::default(),
            Line::styled(
                "Its verifier is retired, so it cannot certify a new proposal. Recorded evidence is on Proposals, Receipt and Audit.",
                theme::MUTED,
            ),
            Line::default(),
            Line::from(ui::keycap("p", "switch to the Demo gate for a live run")),
        ];
        f.render_widget(
            Paragraph::new(lines).block(ui::card("Run is disabled", theme::BORDER_WARN)),
            area,
        );
        return;
    }
    let busy = app.running || app.output.is_some();
    if area.width >= 112 {
        let [left, right] =
            Layout::horizontal([Constraint::Length(44), Constraint::Min(50)]).areas(area);
        scenarios(f, left, app);
        main(f, right, app, busy);
    } else if busy || app.wizard.confirmation.is_some() {
        main(f, area, app, busy);
    } else {
        let [top, rest] = Layout::vertical([Constraint::Length(3), Constraint::Min(8)]).areas(area);
        selector(f, top, app);
        main(f, rest, app, busy);
    }
}

fn main(f: &mut Frame, area: Rect, app: &App, busy: bool) {
    if busy {
        run_panel(f, area, app);
    } else if let Some(confirmation) = &app.wizard.confirmation {
        review(f, area, app, confirmation);
    } else {
        wizard(f, area, app);
    }
}

fn current_impl(app: &App) -> Option<&str> {
    app.snapshot
        .as_ref()
        .and_then(|s| s.implementation.as_deref())
}

/// Second line of a scenario card: what it targets and whether it can run now.
fn scenario_status(p: &Preset, current: Option<&str>) -> Vec<Span<'static>> {
    let mut spans = if p.upgrade {
        match p.availability(current) {
            Availability::Ready => vec![ui::text(format!("{} ready", theme::DOT), theme::GOOD)],
            Availability::Done => vec![ui::text(format!("{} applied", theme::PASS), theme::MUTED)],
            Availability::RequiresPrevious => {
                vec![ui::text("needs the previous upgrade", theme::WARN)]
            }
            Availability::Unknown => vec![ui::text("checking the proxy…", theme::DIM)],
        }
    } else {
        vec![ui::text(
            p.rule.map_or_else(
                || "must be refused".to_owned(),
                |r| format!("Rule {r} · must be refused"),
            ),
            theme::MUTED,
        )]
    };
    if let Some(previous) = &p.previous_run {
        spans.push(ui::text(
            format!(" · {}", ui::seconds(previous.elapsed_ms)),
            theme::DIM,
        ));
    }
    spans
}

fn scenarios(f: &mut Frame, area: Rect, app: &App) {
    let block = ui::card("Scenarios", theme::BORDER);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let current = current_impl(app);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut selected_rows = (0, 0);
    let mut last_kind = None;
    for (i, p) in app.presets.iter().enumerate() {
        if last_kind != Some(p.upgrade) {
            if last_kind.is_some() {
                lines.push(Line::default());
            }
            lines.push(Line::styled(
                if p.upgrade {
                    "HONEST UPGRADES"
                } else {
                    "MUST BE REFUSED"
                },
                theme::DIM,
            ));
            last_kind = Some(p.upgrade);
        }
        let chosen = i == app.wizard.preset;
        let title_width = usize::from(inner.width).saturating_sub(2);
        let mut title = Line::from(vec![
            if chosen {
                ui::text(format!("{} ", theme::SELECTOR), theme::TITLE)
            } else {
                Span::raw("  ")
            },
            ui::text(
                ui::clip(&p.title, title_width),
                if chosen { theme::STRONG } else { theme::MUTED },
            ),
        ]);
        let mut status_spans = vec![Span::raw("  ")];
        status_spans.extend(scenario_status(p, current));
        let mut status = Line::from(status_spans);
        if chosen {
            selected_rows = (lines.len(), lines.len() + 2);
            title = title.style(theme::SELECTED_ROW);
            status = status.style(theme::SELECTED_ROW);
        }
        lines.push(title);
        lines.push(status);
    }
    if lines.is_empty() {
        lines.push(Line::styled("No scenarios loaded.", theme::MUTED));
    }
    let height = usize::from(inner.height);
    let scroll = selected_rows.1.saturating_sub(height);
    let (visible, offset) = ui::window(&lines, height, scroll as u16);
    f.render_widget(Paragraph::new(visible), inner);
    ui::scrollbar(f, inner, lines.len(), height, offset);
}

/// Narrow layouts: one row to pick the scenario.
fn selector(f: &mut Frame, area: Rect, app: &App) {
    let Some(p) = app.presets.get(app.wizard.preset) else {
        return;
    };
    let mut spans = vec![
        ui::text("◀ ▶  ", theme::ACCENT),
        ui::text(
            format!("{}/{}  ", app.wizard.preset + 1, app.presets.len()),
            theme::DIM,
        ),
        ui::text(
            ui::clip(&p.title, usize::from(area.width).saturating_sub(40)),
            theme::STRONG,
        ),
        Span::raw("  "),
    ];
    spans.extend(scenario_status(p, current_impl(app)));
    f.render_widget(
        Paragraph::new(Line::from(spans)).block(ui::card("Scenario", theme::BORDER)),
        area,
    );
}

fn flow(app: &App) -> Line<'static> {
    let signer_ready = !app.wizard.account.is_empty() && !app.wizard.key_file.is_empty();
    let current = if signer_ready { 2 } else { 0 };
    let names = ["Signer", "Scenario", "Review", "Authorize"];
    let mut spans = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let (icon, style) = match i.cmp(&current) {
            std::cmp::Ordering::Less => (theme::PASS, theme::GOOD),
            std::cmp::Ordering::Equal => (theme::DOT, theme::TITLE),
            std::cmp::Ordering::Greater => (theme::PENDING, theme::DIM),
        };
        spans.push(ui::text(format!("{icon} {}  ", name), style));
        if i + 1 < names.len() {
            spans.push(ui::text("── ", theme::BORDER));
        }
    }
    Line::from(spans)
}

fn wizard(f: &mut Frame, area: Rect, app: &App) {
    let width = ui::inner_width(area.width);
    let preset = app.presets.get(app.wizard.preset);
    let mut lines: Vec<Line<'static>> = vec![flow(app), Line::default()];
    let upgrade = app.wizard.upgrade;
    let mut head = vec![ui::text(
        preset.map_or_else(|| "Custom bundle".to_owned(), |p| p.title.clone()),
        theme::STRONG,
    )];
    head.push(Span::raw("  "));
    head.push(if upgrade {
        ui::chip("UPGRADE", theme::CHIP_GOOD)
    } else {
        ui::chip("ATTACK", theme::CHIP_ACCENT)
    });
    lines.push(Line::from(head));
    let expectation = if upgrade {
        "All five rules must pass. The gate then signs, the signature is relayed to Base Sepolia and the proxy moves to the new implementation."
    } else {
        "The verifier must refuse this upgrade. The gate records the refusal, nothing is signed and the proxy stays exactly as it is."
    };
    lines.extend(
        ui::wrap(expectation, width)
            .into_iter()
            .map(|l| Line::styled(l, theme::MUTED)),
    );
    if let Some(p) = preset {
        let note = match p.availability(current_impl(app)) {
            Availability::Done => {
                Some("This upgrade is already applied to the proxy; choose another scenario.")
            }
            Availability::RequiresPrevious => Some(
                "Apply the previous honest upgrade first; this one requires it as the current implementation.",
            ),
            _ => None,
        };
        if let Some(note) = note {
            lines.push(Line::styled(format!("{} {note}", theme::FAIL), theme::WARN));
        }
        if let Some(previous) = &p.previous_run {
            lines.push(Line::styled(
                format!(
                    "Last recorded run took {}{}.",
                    ui::seconds(previous.elapsed_ms),
                    previous
                        .observed_at
                        .as_deref()
                        .map_or(String::new(), |t| format!(" ({})", ui::time_of_day(t)))
                ),
                theme::DIM,
            ));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(vec![
        ui::text("INPUTS", theme::DIM),
        ui::text("  e to edit", theme::DIM),
    ]));
    let value_width = width.saturating_sub(FORM_LABEL + 2);
    for (i, (label, value)) in FORM_LABELS.iter().zip(app.wizard.fields()).enumerate() {
        let focused = app.wizard.field == i;
        let editing = app.wizard.editing && focused;
        let shown = if editing {
            format!(
                "{}{}",
                ui::clip_start(value, value_width.saturating_sub(1)),
                theme::CURSOR
            )
        } else if value.is_empty() {
            match i {
                0 | 1 => "required".to_owned(),
                _ => "—".to_owned(),
            }
        } else {
            ui::fit(value, value_width)
        };
        let value_style = if editing {
            theme::INPUT
        } else if value.is_empty() {
            theme::DIM
        } else {
            theme::BASE
        };
        let marker = if focused {
            ui::text(format!("{} ", theme::SELECTOR), theme::TITLE)
        } else {
            Span::raw("  ")
        };
        let padded = if editing {
            format!("{shown:<value_width$}")
        } else {
            shown
        };
        lines.push(Line::from(vec![
            marker,
            ui::text(
                format!("{label:<FORM_LABEL$}"),
                if focused { theme::BASE } else { theme::MUTED },
            ),
            ui::text(padded, value_style),
        ]));
    }
    lines.push(Line::styled(
        format!("  {}", "Key-file contents are never read onto the screen."),
        theme::DIM,
    ));
    lines.push(Line::default());
    lines.push(Line::styled("PIPELINE", theme::DIM));
    lines.extend(
        ui::wrap(&STAGES.join(" → "), width)
            .into_iter()
            .map(|l| Line::styled(l, theme::MUTED)),
    );
    if !upgrade {
        lines.push(Line::styled(
            "An attack stops at the gate verdict; Signature and Base relay never run.",
            theme::DIM,
        ));
    }
    lines.push(Line::default());
    if app.preparing {
        lines.push(Line::styled(
            format!(
                "{} Validating the source and reading live credit, fees and proxy state…",
                theme::spinner(app.now_ns / 1_000_000)
            ),
            theme::ACCENT,
        ));
    } else if app.wizard.editing {
        lines.push(Line::styled(
            "Type or paste · Tab / Shift+Tab changes field · Enter finishes · Esc steps out",
            theme::ACCENT,
        ));
    } else {
        lines.push(Line::from(
            [
                ui::keycap("Enter", "review the write"),
                ui::keycap("u", "attack ⇄ upgrade (custom)"),
            ]
            .concat(),
        ));
        lines.push(Line::styled(
            "Nothing is signed or spent until you press y on the review screen.",
            theme::WARN,
        ));
    }
    let block = ui::card("Run · set up", theme::BORDER_FOCUS);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines = ui::compact(lines, usize::from(inner.height));
    let (visible, offset) = ui::window(&lines, usize::from(inner.height), app.scroll);
    f.render_widget(Paragraph::new(visible), inner);
    ui::scrollbar(f, inner, lines.len(), usize::from(inner.height), offset);
}

fn review(f: &mut Frame, area: Rect, app: &App, confirmation: &str) {
    let width = ui::inner_width(area.width);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for raw in confirmation.lines() {
        let split = raw
            .find(": ")
            .filter(|i| *i <= 22 && !raw[..*i].contains('.'));
        let Some(i) = split else {
            let style = if raw.starts_with("Press y") {
                theme::WARN
            } else {
                theme::MUTED
            };
            lines.extend(
                ui::wrap(raw, width)
                    .into_iter()
                    .map(|l| Line::styled(l, style)),
            );
            continue;
        };
        let (label, value) = (&raw[..i], &raw[i + 2..]);
        let label_width = 21;
        let value_style =
            if value.contains("NON-refundable") || label.starts_with("Proposal storage") {
                theme::WARN
            } else if label.starts_with("Sign as") {
                theme::STRONG
            } else {
                theme::BASE
            };
        for (n, piece) in ui::wrap(value, width.saturating_sub(label_width).max(10))
            .into_iter()
            .enumerate()
        {
            lines.push(ui::kv(
                if n == 0 { label } else { "" },
                label_width,
                vec![ui::text(piece, value_style)],
            ));
        }
    }
    let block = ui::card("Review · nothing is signed yet", theme::BORDER_WARN);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [body, hint] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    let (visible, offset) = ui::window(&lines, usize::from(body.height), app.scroll);
    f.render_widget(Paragraph::new(visible), body);
    ui::scrollbar(f, body, lines.len(), usize::from(body.height), offset);
    f.render_widget(
        Paragraph::new(Line::from(
            [
                ui::keycap("y", "authorize this exact write"),
                ui::keycap("Esc", "go back"),
            ]
            .concat(),
        )),
        hint,
    );
}

fn step_glyph(step: &Step, elapsed_ms: u64) -> (String, Style, Style) {
    match step.state {
        StepState::Pending => (theme::PENDING.into(), theme::DIM, theme::DIM),
        StepState::Active => (
            theme::spinner(elapsed_ms).into(),
            theme::TITLE,
            theme::STRONG,
        ),
        StepState::Pass => (theme::PASS.into(), theme::GOOD, theme::BASE),
        StepState::Refused => (theme::REFUSED.into(), theme::TITLE, theme::TITLE),
        StepState::Error => (theme::FAIL.into(), theme::BAD, theme::BAD),
        StepState::NotRun => (theme::NOT_RUN.into(), theme::DIM, theme::DIM),
    }
}

/// Headline for the run: what is happening or how it ended, in the system's own terms.
fn banner(app: &App) -> (Style, Vec<Line<'static>>) {
    let total = ui::seconds(app.elapsed_ms);
    let preset = app
        .presets
        .get(app.wizard.preset)
        .map_or("custom bundle", |p| p.title.as_str());
    if app.running {
        let active = app
            .steps
            .iter()
            .find(|s| s.state == StepState::Active)
            .map_or("starting", |s| s.name);
        if app.cancelling {
            return (
                theme::BORDER_WARN,
                vec![
                    Line::styled(
                        format!("Cancellation requested · waiting for the worker · {total}"),
                        theme::WARN,
                    ),
                    Line::styled("On-chain steps already taken are not undone.", theme::DIM),
                ],
            );
        }
        return (
            theme::BORDER_FOCUS,
            vec![
                Line::from(vec![
                    ui::text(format!("{} ", theme::spinner(app.elapsed_ms)), theme::TITLE),
                    ui::text("Running ", theme::STRONG),
                    ui::text(format!("· {active} · {total}"), theme::MUTED),
                ]),
                Line::styled(format!("{preset}. Ctrl+C cancels."), theme::DIM),
            ],
        );
    }
    let step_detail = |s: &Step| ui::clip(&s.detail, 400);
    if let Some(s) = app.steps.iter().find(|s| s.state == StepState::Refused) {
        return (
            theme::BORDER_FOCUS,
            vec![
                Line::from(vec![
                    ui::chip("REFUSED", theme::CHIP_ACCENT),
                    ui::text(
                        format!(
                            "  at {} · no signature produced · proxy unchanged · {total}",
                            s.name
                        ),
                        theme::STRONG,
                    ),
                ]),
                Line::styled(step_detail(s), theme::MUTED),
            ],
        );
    }
    if let Some(s) = app.steps.iter().find(|s| s.state == StepState::Error) {
        return (
            theme::BORDER_BAD,
            vec![
                Line::from(vec![
                    ui::chip("STOPPED", theme::CHIP_BAD),
                    ui::text(format!("  at {} · {total}", s.name), theme::STRONG),
                ]),
                Line::styled(
                    format!(
                        "{} Inspect the artifacts before any retry; never create a duplicate proposal.",
                        step_detail(s)
                    ),
                    theme::WARN,
                ),
            ],
        );
    }
    if app.steps.last().is_some_and(|s| s.state == StepState::Pass) {
        let detail = app.steps.last().map(step_detail).unwrap_or_default();
        return (
            theme::BORDER_GOOD,
            vec![
                Line::from(vec![
                    ui::chip("COMPLETE", theme::CHIP_GOOD),
                    ui::text(format!("  every stage passed · {total}"), theme::STRONG),
                ]),
                Line::styled(detail, theme::MUTED),
            ],
        );
    }
    (
        theme::BORDER_WARN,
        vec![
            Line::from(vec![
                ui::chip("NO OUTCOME", theme::CHIP_WARN),
                ui::text(
                    format!("  the run ended without an observed outcome · {total}"),
                    theme::STRONG,
                ),
            ]),
            Line::styled(
                "Inspect the retained artifacts before preparing another run.",
                theme::MUTED,
            ),
        ],
    )
}

fn run_panel(f: &mut Frame, area: Rect, app: &App) {
    let (border, lines) = banner(app);
    let width = ui::inner_width(area.width);
    let mut banner_lines = Vec::new();
    for (n, line) in lines.into_iter().enumerate() {
        if n == 0 {
            banner_lines.push(line);
        } else {
            // Detail lines wrap onto at most two rows so the pipeline stays visible.
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let style = line.spans.first().map(|s| s.style).unwrap_or(theme::MUTED);
            banner_lines.extend(
                ui::wrap(&text, width)
                    .into_iter()
                    .take(2)
                    .map(|l| Line::styled(l, style)),
            );
        }
    }
    let banner_height = banner_lines.len() as u16 + 2;
    let [top, rest] =
        Layout::vertical([Constraint::Length(banner_height), Constraint::Min(6)]).areas(area);
    f.render_widget(
        Paragraph::new(banner_lines).block(ui::card("Run", border)),
        top,
    );
    let [pipeline, evidence] = if rest.width >= 84 {
        Layout::horizontal([Constraint::Length(44), Constraint::Min(40)]).areas(rest)
    } else {
        let h = (app.steps.len() as u16 + 2).min(rest.height.saturating_sub(5));
        Layout::vertical([Constraint::Length(h), Constraint::Min(4)]).areas(rest)
    };
    pipeline_card(f, pipeline, app);
    evidence_card(f, evidence, app);
}

fn pipeline_card(f: &mut Frame, area: Rect, app: &App) {
    let block = ui::card("Pipeline", theme::BORDER);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let name_width = app
        .steps
        .iter()
        .map(|s| ui::cols(s.name))
        .max()
        .unwrap_or(0);
    let lines: Vec<Line<'static>> = app
        .steps
        .iter()
        .map(|step| {
            let (icon, icon_style, name_style) = step_glyph(step, app.elapsed_ms);
            let time = match step.state {
                StepState::Pending | StepState::NotRun => String::new(),
                _ => ui::seconds(step.elapsed_ms),
            };
            let pad = usize::from(inner.width)
                .saturating_sub(2 + name_width + ui::cols(&time))
                .max(1);
            Line::from(vec![
                ui::text(format!("{icon} "), icon_style),
                ui::text(format!("{:<name_width$}", step.name), name_style),
                ui::text(" ".repeat(pad), theme::BASE),
                ui::text(time, theme::DIM),
            ])
        })
        .collect();
    let (visible, offset) = ui::window(&lines, usize::from(inner.height), 0);
    f.render_widget(Paragraph::new(visible), inner);
    ui::scrollbar(f, inner, lines.len(), usize::from(inner.height), offset);
}

fn evidence_card(f: &mut Frame, area: Rect, app: &App) {
    let block = ui::card("Evidence", theme::BORDER);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = usize::from(inner.width);
    let height = usize::from(inner.height);
    let mut head: Vec<Line<'static>> = Vec::new();
    head.push(ui::kv(
        "Artifacts",
        11,
        vec![ui::text(
            ui::clip(
                app.output.as_deref().unwrap_or("reserving directory…"),
                width.saturating_sub(11),
            ),
            theme::MUTED,
        )],
    ));
    if let Some(start) = &app.run_started_at {
        head.push(ui::kv(
            "Started",
            11,
            vec![ui::text(
                format!(
                    "{}{}",
                    ui::time_of_day(start),
                    app.run_finished_at
                        .as_deref()
                        .map_or(String::new(), |t| format!(
                            " · finished {}",
                            ui::time_of_day(t)
                        ))
                ),
                theme::MUTED,
            )],
        ));
    }
    for step in app.steps.iter().filter(|s| !s.detail.is_empty()) {
        let (_, icon_style, _) = step_glyph(step, app.elapsed_ms);
        head.push(Line::styled(step.name.to_owned(), icon_style));
        head.extend(
            ui::wrap(&step.detail, width.saturating_sub(2))
                .into_iter()
                .map(|l| Line::styled(format!("  {l}"), theme::MUTED)),
        );
    }
    let head_cap = (height * 3 / 5).max(3);
    head.truncate(head_cap);
    let mut lines = head;
    if !app.run_log.is_empty() && lines.len() + 2 <= height {
        lines.push(Line::styled("LOG", theme::DIM));
        let room = height - lines.len();
        let skip = app.run_log.len().saturating_sub(room);
        lines.extend(
            app.run_log
                .iter()
                .skip(skip)
                .map(|l| Line::styled(ui::clip(l, width), theme::DIM)),
        );
    }
    f.render_widget(Paragraph::new(lines), inner);
}
