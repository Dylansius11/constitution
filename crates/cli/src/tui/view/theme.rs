//! Every color, style and glyph of the terminal UI. No other module names a color.
//!
//! The palette follows the web app: night background, lavender accent, green for verified
//! success, amber for cost and caution, red for failure. A refusal is the system working, so it
//! is drawn in the accent color rather than the error color.
use ratatui::style::{Color, Modifier, Style};

const NIGHT: Color = Color::Rgb(18, 16, 25);
const HILITE: Color = Color::Rgb(44, 38, 64);
const INK: Color = Color::Rgb(237, 235, 245);
const MUTED_INK: Color = Color::Rgb(169, 165, 184);
const DIM_INK: Color = Color::Rgb(112, 108, 130);
const LAVENDER: Color = Color::Rgb(183, 148, 246);
const GREEN: Color = Color::Rgb(95, 211, 154);
const RED: Color = Color::Rgb(255, 107, 107);
const AMBER: Color = Color::Rgb(240, 180, 70);
const LINE: Color = Color::Rgb(58, 55, 72);

pub const BASE: Style = Style::new().fg(INK).bg(NIGHT);
pub const STRONG: Style = Style::new().fg(INK).add_modifier(Modifier::BOLD);
pub const TITLE: Style = Style::new().fg(LAVENDER).add_modifier(Modifier::BOLD);
pub const ACCENT: Style = Style::new().fg(LAVENDER);
pub const MUTED: Style = Style::new().fg(MUTED_INK);
pub const DIM: Style = Style::new().fg(DIM_INK);
pub const GOOD: Style = Style::new().fg(GREEN);
pub const BAD: Style = Style::new().fg(RED);
pub const WARN: Style = Style::new().fg(AMBER);
pub const BORDER: Style = Style::new().fg(LINE);
pub const BORDER_FOCUS: Style = Style::new().fg(LAVENDER);
pub const BORDER_GOOD: Style = Style::new().fg(GREEN);
pub const BORDER_WARN: Style = Style::new().fg(AMBER);
pub const BORDER_BAD: Style = Style::new().fg(RED);
/// Background of the selected row, applied over a whole line.
pub const SELECTED_ROW: Style = Style::new().bg(HILITE);
/// Focused input field.
pub const INPUT: Style = Style::new().fg(INK).bg(HILITE);
pub const TAB_ACTIVE: Style = Style::new()
    .fg(NIGHT)
    .bg(LAVENDER)
    .add_modifier(Modifier::BOLD);
/// Keycap in the footer and the help overlay.
pub const KEYCAP: Style = Style::new().fg(INK).bg(HILITE).add_modifier(Modifier::BOLD);

const fn chip(bg: Color) -> Style {
    Style::new().fg(NIGHT).bg(bg).add_modifier(Modifier::BOLD)
}
pub const CHIP_ACCENT: Style = chip(LAVENDER);
pub const CHIP_GOOD: Style = chip(GREEN);
pub const CHIP_BAD: Style = chip(RED);
pub const CHIP_WARN: Style = chip(AMBER);
pub const CHIP_MUTED: Style = chip(MUTED_INK);

pub const PENDING: &str = "○";
pub const PASS: &str = "✓";
pub const FAIL: &str = "✗";
pub const REFUSED: &str = "◆";
pub const NOT_RUN: &str = "–";
pub const DOT: &str = "●";
pub const CURSOR: &str = "▏";
pub const SELECTOR: &str = "▸";
pub const ELLIPSIS: &str = "…";
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Braille spinner frame derived from elapsed milliseconds, so rendering stays pure.
pub fn spinner(elapsed_ms: u64) -> &'static str {
    SPINNER[(elapsed_ms / 90) as usize % SPINNER.len()]
}
