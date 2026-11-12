use super::{panel, theme};
use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Paragraph, Wrap},
};
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let areas = Layout::vertical([Constraint::Length(4), Constraint::Min(2)]).split(area);
    f.render_widget(Paragraph::new(format!("Proposal id [i edits, Enter fetches]: {}{}\nNew JSON path [s edits, Enter saves]: {}{}\n{}",app.receipt_id,if app.receipt_editing {" ◀ editing"}else{""},app.receipt_path,if app.receipt_save_editing {" ◀ editing"}else{""},if app.receipt_loading {"Fetching observed public state…"}else{"j/k scroll · saved JSON is create-only; no source or wallet secrets"})).style(theme::TITLE),areas[0]);
    let text = app.receipt.as_ref().map_or_else(||"No receipt fetched. Enter a proposal id to assemble constitution.receipt/1 from observed public state. Certificate-only facts remain absent unless verified evidence exists.".into(),|r|serde_json::to_string_pretty(r).unwrap_or_else(|e|e.to_string()));
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((app.receipt_scroll, 0))
            .block(panel("constitution.receipt/1")),
        areas[1],
    );
}
