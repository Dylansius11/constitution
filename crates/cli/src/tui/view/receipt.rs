use super::{theme, ui};
use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

const LABEL: usize = 14;

fn input(
    label: &str,
    value: &str,
    editing: bool,
    placeholder: &str,
    width: usize,
) -> Line<'static> {
    let shown = if editing {
        format!(
            "{:<width$}",
            format!(
                "{}{}",
                ui::clip_start(value, width.saturating_sub(1)),
                theme::CURSOR
            )
        )
    } else if value.is_empty() {
        placeholder.to_owned()
    } else {
        ui::clip(value, width)
    };
    let style = if editing {
        theme::INPUT
    } else if value.is_empty() {
        theme::DIM
    } else {
        theme::BASE
    };
    Line::from(vec![
        Span::styled(
            format!("{} ", if editing { theme::SELECTOR } else { " " }),
            theme::TITLE,
        ),
        ui::text(format!("{label:<LABEL$}"), theme::MUTED),
        ui::text(shown, style),
    ])
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let [form, result] = Layout::vertical([Constraint::Length(6), Constraint::Min(4)]).areas(area);
    let width = ui::inner_width(area.width).saturating_sub(LABEL + 2);

    let status = if app.receipt_loading {
        Line::styled(
            format!(
                "{} Fetching observed public state…",
                theme::spinner(app.now_ns / 1_000_000)
            ),
            theme::ACCENT,
        )
    } else if app.receipt_editing || app.receipt_save_editing {
        Line::styled("Type · Enter confirms · Esc cancels", theme::ACCENT)
    } else {
        Line::from(
            [
                ui::keycap("i", "proposal id"),
                ui::keycap("Enter", "fetch"),
                ui::keycap("s", "save JSON"),
            ]
            .concat(),
        )
    };
    f.render_widget(
        Paragraph::new(vec![
            input(
                "Proposal id",
                &app.receipt_id,
                app.receipt_editing,
                "digits only, for example 8",
                width,
            ),
            input(
                "Save to",
                &app.receipt_path,
                app.receipt_save_editing,
                "path of a new .json file (create-only)",
                width,
            ),
            Line::default(),
            status,
        ])
        .block(ui::card("Receipt", theme::BORDER_FOCUS)),
        form,
    );

    let block = ui::card("constitution.receipt/1", theme::BORDER);
    let inner = block.inner(result);
    f.render_widget(block, result);
    let Some(receipt) = &app.receipt else {
        let lines = vec![
            Line::styled("No receipt fetched.", theme::MUTED),
            Line::default(),
            Line::styled(
                "Enter a proposal id to assemble constitution.receipt/1 from observed public state.",
                theme::DIM,
            ),
            Line::styled(
                "Certificate-only facts stay absent unless verified evidence exists. Saved files hold no source and no wallet secrets.",
                theme::DIM,
            ),
        ];
        f.render_widget(Paragraph::new(lines), inner);
        return;
    };
    let pretty = serde_json::to_string_pretty(receipt).unwrap_or_else(|e| e.to_string());
    let lines = ui::json_lines(&pretty, usize::from(inner.width).saturating_sub(1));
    let height = usize::from(inner.height);
    let (visible, offset) = ui::window(&lines, height, app.receipt_scroll);
    f.render_widget(Paragraph::new(visible), inner);
    ui::scrollbar(f, inner, lines.len(), height, offset);
}
