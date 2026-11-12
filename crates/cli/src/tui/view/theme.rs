//! All application colors and styles; visual polish can change this module independently.
use ratatui::style::{Color, Modifier, Style};
pub const BASE: Style = Style::new().fg(Color::Gray).bg(Color::Black);
pub const TITLE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
pub const MUTED: Style = Style::new().fg(Color::DarkGray);
pub const SELECTED: Style = Style::new()
    .fg(Color::Black)
    .bg(Color::Cyan)
    .add_modifier(Modifier::BOLD);
pub const GOOD: Style = Style::new().fg(Color::Green);
pub const BAD: Style = Style::new().fg(Color::Red);
pub const WARN: Style = Style::new().fg(Color::Yellow);
pub const ACTIVE: Style = Style::new().fg(Color::Cyan);
pub const BORDER: Style = Style::new().fg(Color::DarkGray);
