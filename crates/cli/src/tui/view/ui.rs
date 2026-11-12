//! Layout and text helpers shared by the screens. Pure: no state, no I/O.
use super::theme;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Padding, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

/// Column count of a string; every glyph the UI prints is one column wide.
pub fn cols(s: &str) -> usize {
    s.chars().count()
}

/// Cuts to `max` columns with a trailing ellipsis.
pub fn clip(s: &str, max: usize) -> String {
    if cols(s) <= max {
        return s.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push_str(theme::ELLIPSIS);
    out
}

/// Cuts the front instead, so the end of what a user is typing stays visible.
pub fn clip_start(s: &str, max: usize) -> String {
    let n = cols(s);
    if n <= max {
        return s.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::from(theme::ELLIPSIS);
    out.extend(s.chars().skip(n - (max - 1)));
    out
}

/// Keeps head and tail of an identifier (`0xa98855…8cc8`) when the full value does not fit.
pub fn fit(s: &str, max: usize) -> String {
    let n = cols(s);
    if n <= max {
        return s.to_owned();
    }
    if max <= 8 {
        return clip(s, max);
    }
    let tail = (max - 1) / 3;
    let head = max - 1 - tail;
    let mut out: String = s.chars().take(head).collect();
    out.push_str(theme::ELLIPSIS);
    out.extend(s.chars().skip(n - tail));
    out
}

/// Word wrap that preserves blank lines and hard-splits words longer than `width`.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let mut current = String::new();
        let mut current_cols = 0;
        for word in raw.split_whitespace() {
            let mut chars: Vec<char> = word.chars().collect();
            while chars.len() > width {
                if current_cols > 0 {
                    out.push(std::mem::take(&mut current));
                    current_cols = 0;
                }
                out.push(chars.drain(..width).collect());
            }
            let word_cols = chars.len();
            if word_cols == 0 {
                continue;
            }
            if current_cols == 0 {
                current = chars.into_iter().collect();
                current_cols = word_cols;
            } else if current_cols + 1 + word_cols <= width {
                current.push(' ');
                current.extend(chars);
                current_cols += 1 + word_cols;
            } else {
                out.push(std::mem::take(&mut current));
                current = chars.into_iter().collect();
                current_cols = word_cols;
            }
        }
        out.push(current);
    }
    out
}

/// `00:12:05`, or `1d 02h 03m 04s` once a day or more.
pub fn clock(total_seconds: u64) -> String {
    let (d, h, m, s) = (
        total_seconds / 86_400,
        total_seconds / 3600 % 24,
        total_seconds / 60 % 60,
        total_seconds % 60,
    );
    if d > 0 {
        format!("{d}d {h:02}h {m:02}m {s:02}s")
    } else {
        format!("{h:02}:{m:02}:{s:02}")
    }
}

pub fn seconds(ms: u64) -> String {
    format!("{:.1}s", ms as f64 / 1000.0)
}

/// `04:58:17 UTC` from an RFC 3339 timestamp; the input is returned unchanged if it is shorter.
pub fn time_of_day(utc: &str) -> String {
    utc.get(11..19)
        .map_or_else(|| utc.to_owned(), |t| format!("{t} UTC"))
}

pub fn chip(text: &str, style: Style) -> Span<'static> {
    Span::styled(format!(" {text} "), style)
}

pub fn text(value: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(value.into(), style)
}

/// A rounded card with a lavender title; the caller chooses the border style.
pub fn card(title: &str, border: Style) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title.to_owned(), theme::TITLE),
            Span::raw(" "),
        ]))
        .padding(Padding::horizontal(1))
}

/// Width available to text inside a [`card`] of the given outer width.
pub fn inner_width(outer: u16) -> usize {
    usize::from(outer.saturating_sub(4))
}

/// A muted label padded to `label_width`, then the value spans.
pub fn kv(label: &str, label_width: usize, value: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![Span::styled(format!("{label:<label_width$}"), theme::MUTED)];
    spans.extend(value);
    Line::from(spans)
}

/// A footer or help key hint: a keycap followed by its meaning.
pub fn keycap(key: &str, label: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!(" {key} "), theme::KEYCAP),
        Span::styled(format!(" {label}   "), theme::MUTED),
    ]
}

/// A horizontal rule of `width` columns.
pub fn rule(width: usize, style: Style) -> Line<'static> {
    Line::styled("─".repeat(width), style)
}

/// Drops the blank separator rows when the content would not otherwise fit `height` rows.
pub fn compact(lines: Vec<Line<'static>>, height: usize) -> Vec<Line<'static>> {
    if lines.len() <= height {
        return lines;
    }
    lines.into_iter().filter(|l| l.width() > 0).collect()
}

/// Cuts `lines` to a window of `height` rows starting at `scroll`, clamped so the last line can
/// always be reached. Returns the window and the clamped offset.
pub fn window<T: Clone>(lines: &[T], height: usize, scroll: u16) -> (Vec<T>, usize) {
    let max = lines.len().saturating_sub(height);
    let offset = usize::from(scroll).min(max);
    (
        lines.iter().skip(offset).take(height).cloned().collect(),
        offset,
    )
}

/// Draws the window scrollbar at the right edge of `area` when `total` rows overflow `height`.
pub fn scrollbar(f: &mut Frame, area: Rect, total: usize, height: usize, offset: usize) {
    if total <= height || area.width == 0 {
        return;
    }
    let mut state = ScrollbarState::new(total.saturating_sub(height)).position(offset);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .track_style(theme::BORDER)
            .thumb_symbol("┃")
            .thumb_style(theme::ACCENT),
        area,
        &mut state,
    );
}

/// Draws tall content into an off-screen buffer and blits the visible window, so cards can be
/// laid out at their full height and still scroll. `draw` receives the buffer and the rectangle
/// (full content size, origin 0,0) to fill. Returns nothing; the scrollbar is drawn when needed.
pub fn scrolled(
    f: &mut Frame,
    area: Rect,
    scroll: u16,
    total: u16,
    draw: impl FnOnce(&mut Buffer, Rect),
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if total <= area.height {
        draw(f.buffer_mut(), Rect::new(area.x, area.y, area.width, total));
        return;
    }
    let width = area.width.saturating_sub(1).max(1);
    let content = Rect::new(0, 0, width, total);
    let mut buffer = Buffer::empty(content);
    draw(&mut buffer, content);
    let offset = scroll.min(total - area.height);
    for y in 0..area.height {
        for x in 0..width {
            if let Some(cell) = buffer.cell((x, offset + y)).cloned()
                && let Some(target) = f.buffer_mut().cell_mut((area.x + x, area.y + y))
            {
                *target = cell;
            }
        }
    }
    scrollbar(
        f,
        area,
        usize::from(total),
        usize::from(area.height),
        usize::from(offset),
    );
}

/// Pretty-printed JSON as colored, hard-wrapped lines: keys in the accent color.
pub fn json_lines(pretty: &str, width: usize) -> Vec<Line<'static>> {
    let width = width.max(8);
    let mut out = Vec::new();
    for raw in pretty.lines() {
        let indent = raw.len() - raw.trim_start().len();
        let body = raw.trim_start();
        let key_len = body
            .strip_prefix('"')
            .and_then(|rest| rest.find("\": "))
            .map(|i| i + 4);
        let chars: Vec<char> = raw.chars().collect();
        for (n, chunk) in chars.chunks(width).enumerate() {
            let chunk: String = chunk.iter().collect();
            match key_len {
                Some(k) if n == 0 && indent + k <= chunk.chars().count() => {
                    let split: usize = chunk
                        .char_indices()
                        .nth(indent + k)
                        .map_or(chunk.len(), |(i, _)| i);
                    out.push(Line::from(vec![
                        Span::styled(chunk[..split].to_owned(), theme::ACCENT),
                        Span::styled(chunk[split..].to_owned(), theme::BASE),
                    ]));
                }
                _ => out.push(Line::styled(chunk, theme::BASE)),
            }
        }
        if chars.is_empty() {
            out.push(Line::default());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_keeps_blank_lines_and_splits_long_words() {
        assert_eq!(wrap("ab cd\n\nef", 5), ["ab cd", "", "ef"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("aa bbbbbbb", 4), ["aa", "bbbb", "bbb"]);
    }

    #[test]
    fn identifiers_keep_both_ends_and_fit_the_width() {
        let hash = format!("0x{}", "ab".repeat(32));
        let short = fit(&hash, 24);
        assert_eq!(cols(&short), 24);
        assert!(short.starts_with("0xabab") && short.ends_with("abab"));
        assert_eq!(fit("0x12", 24), "0x12");
        assert_eq!(clip_start("abcdefgh", 4), "…fgh");
    }

    #[test]
    fn durations_switch_to_days() {
        assert_eq!(clock(3725), "01:02:05");
        assert_eq!(clock(90_061), "1d 01h 01m 01s");
    }

    #[test]
    fn window_never_scrolls_past_the_last_line() {
        let lines: Vec<u8> = (0..10).collect();
        assert_eq!(window(&lines, 4, 99), (vec![6, 7, 8, 9], 6));
        assert_eq!(window(&lines, 20, 5).0.len(), 10);
    }
}
