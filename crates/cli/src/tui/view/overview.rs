use super::{theme, ui};
use crate::tui::app::{App, AppraisalResult, Snapshot};
use constitution_types::ProposalState;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

const LABEL: usize = 15;

struct Card {
    title: &'static str,
    border: Style,
    lines: Vec<Line<'static>>,
}

impl Card {
    fn height(&self) -> u16 {
        self.lines.len() as u16 + 2
    }
    fn draw(&self, buffer: &mut ratatui::buffer::Buffer, area: Rect) {
        Paragraph::new(self.lines.clone())
            .block(ui::card(self.title, self.border))
            .render(area, buffer);
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(s) = &app.snapshot else {
        let lines = vec![
            Line::styled(
                format!(
                    "{} Reading public state from NEAR and Base Sepolia…",
                    theme::spinner(app.now_ns / 1_000_000)
                ),
                theme::ACCENT,
            ),
            Line::default(),
            Line::styled(
                "No live value has been observed yet, so none is shown.",
                theme::MUTED,
            ),
            Line::styled(
                "If this stays empty, press r, or set NEAR_RPC_URL / BASE_SEPOLIA_RPC_URL.",
                theme::DIM,
            ),
        ];
        f.render_widget(
            Paragraph::new(lines).block(ui::card("Overview", theme::BORDER)),
            area,
        );
        return;
    };
    let [strip, body] = Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).areas(area);
    render_strip(f, strip, app, s);

    let wide = body.width >= 118;
    let column_width = if wide {
        (body.width - 3) / 2
    } else {
        body.width.saturating_sub(1)
    };
    let inner = ui::inner_width(column_width);
    let (left, right) = cards(app, s, inner);
    let (left, right) = if wide {
        (left, right)
    } else {
        (left.into_iter().chain(right).collect(), Vec::new())
    };
    let total = |cards: &[Card]| cards.iter().map(Card::height).sum::<u16>();
    let rows = total(&left).max(total(&right));
    ui::scrolled(f, body, app.scroll, rows, |buffer, area| {
        let mut y = area.y;
        for card in &left {
            card.draw(buffer, Rect::new(area.x, y, column_width, card.height()));
            y += card.height();
        }
        let mut y = area.y;
        for card in &right {
            card.draw(
                buffer,
                Rect::new(area.x + column_width + 2, y, column_width, card.height()),
            );
            y += card.height();
        }
    });
}

fn render_strip(f: &mut Frame, area: Rect, app: &App, s: &Snapshot) {
    let tiles = Layout::horizontal([Constraint::Ratio(1, 4); 4]).split(area);
    let refused = s
        .proposals
        .iter()
        .filter(|p| p.state == ProposalState::Rejected)
        .count();
    let signed = s
        .proposals
        .iter()
        .filter(|p| p.state == ProposalState::Signed)
        .count();
    let (keys_text, keys_style) = match s.access_keys {
        Some(0) => ("0  ✓ keyless".to_owned(), theme::GOOD),
        Some(n) => (format!("{n}  ✗ not keyless"), theme::BAD),
        None => ("unavailable".to_owned(), theme::WARN),
    };
    let mut proposals = format!("{} · {refused} refused", s.proposals.len());
    if signed > 0 {
        proposals.push_str(&format!(" · {signed} signed"));
    }
    let version = version_label(app, s);
    let (rules_text, rules_style) = match (&s.rule_hash, &s.manifest_hash) {
        (Some(a), Some(b)) if a == b => ("✓ matches manifest", theme::GOOD),
        (Some(_), Some(_)) => ("✗ differs from manifest", theme::BAD),
        _ => ("unknown", theme::WARN),
    };
    let tile_values = [
        ("Access keys", keys_text, keys_style),
        ("Proposals", proposals, theme::STRONG),
        ("Vault runs", version.0, version.1),
        ("Rule set", rules_text.to_owned(), rules_style),
    ];
    for (tile, (title, value, style)) in tiles.iter().zip(tile_values) {
        let width = ui::inner_width(tile.width);
        f.render_widget(
            Paragraph::new(Line::styled(ui::clip(&value, width), style))
                .alignment(Alignment::Center)
                .block(ui::card(title, theme::BORDER)),
            *tile,
        );
    }
}

/// `V1` / `V2` / `V3` when the proxy implementation matches a known upgrade, from the presets.
fn version_label(app: &App, s: &Snapshot) -> (String, Style) {
    let Some(current) = s.implementation.as_deref() else {
        return ("unavailable".into(), theme::WARN);
    };
    let upgrades = app.presets.iter().filter(|p| p.upgrade);
    for p in upgrades.clone() {
        if p.implementation.eq_ignore_ascii_case(current) {
            return (p.id.clone(), theme::STRONG);
        }
    }
    // V1 is not in the gallery; it is the implementation every observed proposal expected.
    if upgrades.into_iter().next().is_some()
        && s.proposals
            .iter()
            .any(|p| p.expected_current_impl.eq_ignore_ascii_case(current))
    {
        return ("V1 (initial)".into(), theme::STRONG);
    }
    ("unrecognised".into(), theme::WARN)
}

fn cards(app: &App, s: &Snapshot, inner: usize) -> (Vec<Card>, Vec<Card>) {
    let value = inner.saturating_sub(LABEL);
    let dim_link = |url: &str| {
        vec![ui::text(
            ui::clip(url.trim_start_matches("https://"), value),
            theme::DIM,
        )]
    };

    let keyless = match s.access_keys {
        Some(0) => vec![
            ui::chip("0 KEYLESS", theme::CHIP_GOOD),
            ui::text(" nobody can sign for this gate", theme::MUTED),
        ],
        Some(n) => vec![ui::chip(&format!("{n} KEYS"), theme::CHIP_BAD)],
        None => vec![ui::text("unavailable", theme::WARN)],
    };
    let authority = Card {
        title: "Authority",
        border: theme::BORDER,
        lines: vec![
            ui::kv(
                "Gate",
                LABEL,
                vec![ui::text(ui::clip(&s.gate, value), theme::STRONG)],
            ),
            ui::kv("Access keys", LABEL, keyless),
            ui::kv(
                "Delay",
                LABEL,
                vec![ui::text(
                    ui::clip(
                        &s.delay_days.map_or_else(
                            || "unavailable".to_owned(),
                            |d| format!("{d} day(s) on governance changes"),
                        ),
                        value,
                    ),
                    theme::BASE,
                )],
            ),
            ui::kv("Explorer", LABEL, dim_link(&s.gate_url)),
        ],
    };

    let (binding, binding_style) = match (&s.rule_hash, &s.manifest_hash) {
        (Some(a), Some(b)) if a == b => (" MATCH ", theme::CHIP_GOOD),
        (Some(_), Some(_)) => (" MISMATCH ", theme::CHIP_BAD),
        _ => (" UNKNOWN ", theme::CHIP_WARN),
    };
    let hash = |h: &Option<String>| {
        vec![ui::text(
            h.as_deref()
                .map_or_else(|| "unavailable".to_owned(), |h| ui::fit(h, value)),
            theme::BASE,
        )]
    };
    let mut rules = vec![
        ui::kv("Active", LABEL, hash(&s.rule_hash)),
        ui::kv("Committed", LABEL, hash(&s.manifest_hash)),
        ui::kv("Binding", LABEL, vec![Span::styled(binding, binding_style)]),
        Line::default(),
    ];
    rules.extend(
        ui::wrap(
            "R1 storage layout · R2 solvency · R3 forbidden code · R4 initializer · R5 replay",
            inner,
        )
        .into_iter()
        .map(|l| Line::styled(l, theme::DIM)),
    );
    let rules = Card {
        title: "Rules",
        border: theme::BORDER,
        lines: rules,
    };

    let (version, version_style) = version_label(app, s);
    let vault = Card {
        title: "Vault · Base Sepolia",
        border: theme::BORDER,
        lines: vec![
            ui::kv("Runs", LABEL, vec![ui::text(version, version_style)]),
            ui::kv(
                "Implementation",
                LABEL,
                vec![ui::text(
                    s.implementation
                        .as_deref()
                        .map_or_else(|| "unavailable".to_owned(), |i| ui::fit(i, value)),
                    theme::BASE,
                )],
            ),
            ui::kv("Explorer", LABEL, dim_link(&s.proxy_url)),
        ],
    };

    let verifier = Card {
        title: "Verifier",
        border: theme::BORDER,
        lines: verifier_lines(app, s, inner),
    };
    let timelock = Card {
        title: "Timelock queue",
        border: theme::BORDER,
        lines: timelock_lines(app, s, inner),
    };
    let mut left = vec![authority, rules, vault];
    let guarantee = Card {
        title: "What the gate guarantees",
        border: theme::BORDER,
        lines: ui::wrap(
            "No key can upgrade the vault. The gate signs only for a certificate from the attested verifier, issued against the committed rules R1–R5. The private AI reviewer can only veto.",
            inner,
        )
        .into_iter()
        .map(|l| Line::styled(l, theme::MUTED))
        .collect(),
    };
    let mut right = vec![verifier, timelock, guarantee];
    if !s.errors.is_empty() {
        let mut lines = Vec::new();
        for error in &s.errors {
            lines.extend(
                ui::wrap(error, inner)
                    .into_iter()
                    .map(|l| Line::styled(l, theme::WARN)),
            );
        }
        lines.push(Line::styled(
            "r retries after checking the RPC configuration.",
            theme::DIM,
        ));
        right.push(Card {
            title: "Read errors",
            border: theme::BORDER_WARN,
            lines,
        });
    }
    // The left column is short; keep the columns balanced when read errors push the right longer.
    if left.iter().map(Card::height).sum::<u16>() + 8 < right.iter().map(Card::height).sum::<u16>()
        && let Some(card) = right.pop()
    {
        left.push(card);
    }
    (left, right)
}

fn verifier_lines(app: &App, s: &Snapshot, inner: usize) -> Vec<Line<'static>> {
    let value = inner.saturating_sub(LABEL);
    let (status_chip, status_rest) = match s.verifier.split_once(" — ") {
        Some((head, rest)) => (head.to_owned(), rest.to_owned()),
        None => (s.verifier.clone(), String::new()),
    };
    let chip_style = match status_chip.as_str() {
        "online" => theme::CHIP_GOOD,
        "retired" => theme::CHIP_MUTED,
        _ => theme::CHIP_WARN,
    };
    let mut lines = vec![ui::kv(
        "Status",
        LABEL,
        vec![ui::chip(&status_chip.to_uppercase(), chip_style)],
    )];
    lines.extend(
        ui::wrap(&status_rest, inner)
            .into_iter()
            .filter(|l| !l.is_empty())
            .map(|l| Line::styled(l, theme::DIM)),
    );
    if let Some(identity) = &s.identity {
        let hash = |h: &Option<String>| {
            vec![ui::text(
                h.as_deref()
                    .map_or_else(|| "unavailable".to_owned(), |h| ui::fit(h, value)),
                theme::BASE,
            )]
        };
        lines.push(ui::kv(
            "Source",
            LABEL,
            vec![ui::text(
                format!(
                    "{}{}",
                    if identity.recorded {
                        "recorded evidence"
                    } else {
                        "live endpoint"
                    },
                    identity
                        .observed_at
                        .as_deref()
                        .map_or(String::new(), |t| format!(" · {}", ui::time_of_day(t)))
                ),
                theme::MUTED,
            )],
        ));
        lines.push(ui::kv("Cert key", LABEL, hash(&identity.verifier_pubkey)));
        lines.push(ui::kv("Release", LABEL, hash(&identity.release_hash)));
        lines.push(ui::kv("Compose", LABEL, hash(&identity.compose_hash)));
    }
    lines.push(Line::default());
    lines.extend(appraisal_lines(app, inner));
    lines
}

fn appraisal_lines(app: &App, inner: usize) -> Vec<Line<'static>> {
    if app.appraising {
        return vec![Line::styled(
            format!(
                "{} Appraising the pinned demo identity… no pass is assumed",
                theme::spinner(app.now_ns / 1_000_000)
            ),
            theme::ACCENT,
        )];
    }
    let Some(result) = &app.appraisal else {
        return vec![Line::from(
            [ui::keycap("a", "appraise against the pinned compose hash")].concat(),
        )];
    };
    let (label, chip) = appraisal_summary(result);
    let mut lines = vec![Line::from(vec![
        ui::chip(&label, chip),
        ui::text(
            format!(
                " {}/{} checks · {}",
                result.passed(),
                result.checks.len(),
                result.recorded_date.as_deref().map_or_else(
                    || result
                        .observed_at
                        .as_deref()
                        .map_or_else(|| "capture time not recorded".to_owned(), ui::time_of_day),
                    |d| format!("recorded {d}")
                )
            ),
            theme::MUTED,
        ),
    ])];
    let name = result
        .checks
        .iter()
        .map(|c| ui::cols(&c.name))
        .max()
        .unwrap_or(0);
    let detail = inner.saturating_sub(name + 3);
    for check in &result.checks {
        let (icon, style) = if check.ok {
            (theme::PASS, theme::GOOD)
        } else {
            (theme::FAIL, theme::BAD)
        };
        lines.push(Line::from(vec![
            ui::text(format!("{icon} "), style),
            ui::text(format!("{:<name$} ", check.name), theme::BASE),
            ui::text(ui::clip(&check.detail, detail), theme::DIM),
        ]));
    }
    lines
}

fn appraisal_summary(result: &AppraisalResult) -> (String, Style) {
    if result.recorded {
        ("RECORDED".into(), theme::CHIP_MUTED)
    } else if result.ok() {
        ("APPRAISED".into(), theme::CHIP_GOOD)
    } else {
        ("APPRAISAL FAILED".into(), theme::CHIP_BAD)
    }
}

fn timelock_lines(app: &App, s: &Snapshot, inner: usize) -> Vec<Line<'static>> {
    if s.changes.is_empty() {
        return vec![Line::from(vec![
            ui::text(format!("{} ", theme::PASS), theme::GOOD),
            ui::text("No pending governance changes.", theme::MUTED),
        ])];
    }
    let mut lines = Vec::new();
    for change in &s.changes {
        let (state, style) = if change.matured {
            ("MATURED".to_owned(), theme::CHIP_GOOD)
        } else {
            (ui::clock(change.remaining_seconds), theme::CHIP_WARN)
        };
        lines.push(Line::from(vec![
            ui::text(format!("#{} ", change.id), theme::STRONG),
            ui::text(format!("{} · {} ", change.kind, change.status), theme::BASE),
            ui::chip(&state, style),
        ]));
        lines.push(Line::styled(
            ui::clip(
                &format!(
                    "ETA {} · local-clock countdown, not authorization",
                    change.eta_utc
                ),
                inner,
            ),
            theme::DIM,
        ));
        if app.show_raw_json {
            lines.extend(
                ui::wrap(&change.raw.to_string(), inner)
                    .into_iter()
                    .map(|l| Line::styled(l, theme::DIM)),
            );
        }
    }
    lines
}
