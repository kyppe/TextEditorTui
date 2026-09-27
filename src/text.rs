//! Character-index text helpers and the shared "apply marks to text"
//! renderer used by both the entry list view and the editor view.
//!
//! All cursor/selection/mark positions in this app are *character*
//! offsets (not byte offsets), which keeps insert/delete/selection math
//! simple and correct for any valid UTF-8 text. The one simplification
//! this makes (documented, not hidden): every character is assumed to
//! render as one terminal column, which is wrong for wide characters
//! like CJK or emoji. Fine for a plain-text note-taking tool; worth
//! revisiting if that ever matters.

use crate::entry::Mark;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn byte_offset(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

pub fn insert_char(s: &mut String, char_idx: usize, ch: char) {
    let b = byte_offset(s, char_idx);
    s.insert(b, ch);
}

pub fn insert_str(s: &mut String, char_idx: usize, text: &str) {
    let b = byte_offset(s, char_idx);
    s.insert_str(b, text);
}

/// Removes the character immediately before `char_idx`. No-op at 0.
pub fn remove_before(s: &mut String, char_idx: usize) {
    if char_idx == 0 {
        return;
    }
    let start = byte_offset(s, char_idx - 1);
    let end = byte_offset(s, char_idx);
    s.replace_range(start..end, "");
}

/// Colours for the live editor selection.
///
/// The selection sets a foreground as well as a background, because a mark
/// underneath it carries its own colour — a blue link on a blue selection
/// was unreadable. Forcing both guarantees contrast whatever the text was
/// styled as, and since only the *colours* are replaced, the underline,
/// bold, italic and strikethrough of whatever is selected still show.
const SELECTION_BG: Color = Color::Rgb(58, 66, 92);
const SELECTION_FG: Color = Color::Rgb(236, 239, 246);

/// Background for `/` search hits.
const SEARCH_BG: Color = Color::Yellow;

/// Character offsets at which a *soft* (wrap-induced) line break falls,
/// for a viewport `width` columns wide. Breaks after the last space that
/// fits when there is one, otherwise hard-breaks mid-word.
///
/// Both the renderer (`render_lines`) and the cursor-position math
/// (`visual_row_col`) go through this one function, so the drawn text
/// and the cursor can never disagree about where lines break.
pub fn wrap_positions(text: &str, width: usize) -> Vec<usize> {
    if width == 0 {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut breaks = Vec::new();
    let mut row_start = 0usize;
    let mut last_space: Option<usize> = None;
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == '\n' {
            row_start = i + 1;
            last_space = None;
            i += 1;
            continue;
        }
        if i - row_start + 1 > width {
            // `chars[i]` no longer fits on this row: break before it.
            let brk = match last_space {
                Some(s) if s + 1 > row_start => s + 1,
                _ => i,
            };
            breaks.push(brk);
            row_start = brk;
            last_space = None;
            i = brk;
            continue;
        }
        if chars[i] == ' ' {
            last_space = Some(i);
        }
        i += 1;
    }
    breaks
}

/// Turns absolute-text-position `Mark` ranges (character offsets) plus
/// the raw `text` into styled output, soft-wrapped to `width`, ready for
/// a ratatui widget. This is the one function every renderer that shows
/// formatted entry text goes through, so a new `MarkKind` only needs a
/// `FORMATS` entry to render correctly everywhere.
pub fn render_lines(text: &str, marks: &[Mark], width: usize) -> Vec<Line<'static>> {
    render_lines_sel(text, marks, width, None, &[])
}

/// As `render_lines`, plus the editor's transient overlays: the selection
/// and the current search hits. Both are handled here rather than by
/// post-processing the returned lines, so they can't fall out of step with
/// where the text actually wrapped.
pub fn render_lines_sel(
    text: &str,
    marks: &[Mark],
    width: usize,
    selection: Option<(usize, usize)>,
    search: &[(usize, usize)],
) -> Vec<Line<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let breaks = wrap_positions(text, width);

    let mut boundaries: Vec<usize> = vec![0, len];
    for m in marks {
        boundaries.push(m.start.min(len));
        boundaries.push(m.end.min(len));
    }
    for (i, ch) in chars.iter().enumerate() {
        if *ch == '\n' {
            boundaries.push(i);
            boundaries.push(i + 1);
        }
    }
    boundaries.extend(&breaks);
    if let Some((s, e)) = selection {
        boundaries.push(s.min(len));
        boundaries.push(e.min(len));
    }
    for (s, e) in search {
        boundaries.push((*s).min(len));
        boundaries.push((*e).min(len));
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut next_break = 0usize;

    for w in boundaries.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a >= b {
            continue;
        }
        // A soft break starting exactly here ends the current visual row.
        while next_break < breaks.len() && breaks[next_break] <= a {
            if breaks[next_break] == a {
                lines.push(Line::from(std::mem::take(&mut current)));
            }
            next_break += 1;
        }

        // A literal tab would make the terminal jump to its next tab stop,
        // breaking the one-character-one-column rule that the wrap points,
        // the cursor and the mark offsets are all computed against — text
        // pasted in from elsewhere really does contain them. Displaying it
        // as a single space keeps every offset honest; the stored text is
        // untouched, so yanking or saving still round-trips the real tab.
        let segment: String = chars[a..b]
            .iter()
            .map(|c| if *c == '\t' { ' ' } else { *c })
            .collect();
        if chars[a..b] == ['\n'] {
            lines.push(Line::from(std::mem::take(&mut current)));
            continue;
        }

        let mut style = Style::default();
        for m in marks {
            if m.start <= a && m.end >= b {
                style = style.patch((crate::format::find_by_kind(m.kind).style)());
            }
        }
        if search.iter().any(|(s, e)| *s <= a && *e >= b && s != e) {
            style = style.bg(SEARCH_BG).fg(Color::Black);
        }
        // Selection is painted last so it stays visible over a search hit.
        if let Some((s, e)) = selection {
            if s <= a && e >= b && s != e {
                style = style.bg(SELECTION_BG).fg(SELECTION_FG);
            }
        }
        current.push(Span::styled(segment, style));
    }
    lines.push(Line::from(current));
    lines
}

/// Every visual row as a `(start, end)` character range, `end` excluding
/// the newline that ended the row (if one did).
///
/// This is the list the cursor actually moves through: a long line that the
/// screen shows as three rows is three entries here, so `j`/`k` and `V` can
/// work on what you see rather than on the paragraph you typed. Derived
/// from `wrap_positions`, the same function the renderer uses, so the rows
/// here are exactly the rows on screen.
pub fn visual_rows(text: &str, width: usize) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let breaks = wrap_positions(text, width);
    let mut rows = Vec::new();
    let mut start = 0usize;
    let mut next_break = 0usize;

    for (i, ch) in chars.iter().enumerate() {
        // A soft break starts a new row at `i`; the row before it ends there.
        while next_break < breaks.len() && breaks[next_break] <= i {
            if breaks[next_break] == i {
                rows.push((start, i));
                start = i;
            }
            next_break += 1;
        }
        if *ch == '\n' {
            rows.push((start, i));
            start = i + 1;
        }
    }
    rows.push((start, chars.len()));
    rows
}

/// Index of the visual row holding `cursor`, plus that row's bounds.
///
/// A cursor sitting exactly on a soft break belongs to the *following* row,
/// matching `visual_row_col`, which advances its row at the same point.
fn visual_row_of(rows: &[(usize, usize)], cursor: usize) -> usize {
    rows.iter()
        .rposition(|(start, _)| *start <= cursor)
        .unwrap_or(0)
}

/// Bounds of the visual row the cursor is on — what `V` selects.
pub fn visual_row_bounds(text: &str, cursor: usize, width: usize) -> (usize, usize) {
    let rows = visual_rows(text, width);
    rows[visual_row_of(&rows, cursor)]
}

/// Moves `cursor` by `delta` visual rows, keeping its column where the new
/// row is long enough. Clamped to the first and last row rather than
/// refusing to move, so a Page Up near the top still lands at the top.
pub fn visual_move(text: &str, cursor: usize, width: usize, delta: i32) -> usize {
    let rows = visual_rows(text, width);
    let current = visual_row_of(&rows, cursor);
    let col = cursor.saturating_sub(rows[current].0);
    let target = (current as i32 + delta).clamp(0, rows.len() as i32 - 1) as usize;
    let (start, end) = rows[target];
    (start + col).min(end)
}

/// Visual row/column of a character offset once soft wrapping at `width`
/// is taken into account — i.e. where the terminal cursor goes. Distinct
/// from `row_col`, which is about *logical* lines (what `j`/`k` move
/// between); don't mix them up.
pub fn visual_row_col(text: &str, char_idx: usize, width: usize) -> (u16, u16) {
    let breaks = wrap_positions(text, width);
    let mut next_break = 0usize;
    let mut row = 0u16;
    let mut col = 0u16;

    for (i, ch) in text.chars().enumerate() {
        while next_break < breaks.len() && breaks[next_break] == i {
            row += 1;
            col = 0;
            next_break += 1;
        }
        if i == char_idx {
            return (row, col);
        }
        if ch == '\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    // Cursor sitting at the very end of the text: if it has just filled
    // the last column, it belongs at the start of the next row.
    if width > 0 && col as usize >= width {
        row += 1;
        col = 0;
    }
    (row, col)
}

/// Row/column (0-indexed) of a character offset in *logical* lines
/// (newline-separated), ignoring soft wrapping — used for cursor motion.
pub fn row_col(text: &str, char_idx: usize) -> (u16, u16) {
    let mut row = 0u16;
    let mut col = 0u16;
    for ch in text.chars().take(char_idx) {
        if ch == '\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (row, col)
}

/// Inverse of `row_col`: the character offset at a given row/column,
/// clamping `col` to that row's length (used for up/down cursor motion).
pub fn char_index_at(text: &str, row: u16, col: u16) -> usize {
    let mut idx = 0usize;
    for (cur_row, line) in text.split('\n').enumerate() {
        let line_len = char_len(line);
        if cur_row as u16 == row {
            let c = (col as usize).min(line_len);
            return idx + c;
        }
        idx += line_len + 1; // +1 for the '\n'
    }
    // row beyond the last line: clamp to end of text.
    char_len(text)
}

/// Removes the character range `[start, end)`.
pub fn remove_range(s: &mut String, start: usize, end: usize) {
    if start >= end {
        return;
    }
    let a = byte_offset(s, start);
    let b = byte_offset(s, end);
    s.replace_range(a..b, "");
}

// --- Vim-style motions -------------------------------------------------
//
// Words here are whitespace-delimited runs, i.e. Vim's `W`/`B`/`E`
// behaviour rather than `w`/`b`/`e`'s punctuation-aware classes. For a
// plain-text note-taking tool that's the less surprising of the two, and
// it keeps these functions simple enough to verify at a glance.

/// Start of the next word (Vim `w`).
pub fn next_word_start(text: &str, cursor: usize) -> usize {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut i = cursor.min(n);
    while i < n && !c[i].is_whitespace() {
        i += 1;
    }
    while i < n && c[i].is_whitespace() {
        i += 1;
    }
    i
}

/// Start of the previous word (Vim `b`).
pub fn prev_word_start(text: &str, cursor: usize) -> usize {
    let c: Vec<char> = text.chars().collect();
    let mut i = cursor.min(c.len());
    if i == 0 {
        return 0;
    }
    i -= 1;
    while i > 0 && c[i].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !c[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

/// Last character of the current or next word (Vim `e`).
pub fn word_end(text: &str, cursor: usize) -> usize {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    if n == 0 {
        return 0;
    }
    let mut i = (cursor + 1).min(n.saturating_sub(1));
    while i < n && c[i].is_whitespace() {
        i += 1;
    }
    while i + 1 < n && !c[i + 1].is_whitespace() {
        i += 1;
    }
    i.min(n.saturating_sub(1))
}

/// First non-whitespace character of the cursor's line (Vim `^`).
pub fn first_non_blank(text: &str, cursor: usize) -> usize {
    let (start, end) = line_bounds(text, cursor);
    let c: Vec<char> = text.chars().collect();
    let mut i = start;
    while i < end && c[i].is_whitespace() {
        i += 1;
    }
    i
}

/// Character range `[start, end)` of the logical line containing `cursor`,
/// excluding its trailing newline.
pub fn line_bounds(text: &str, cursor: usize) -> (usize, usize) {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let cursor = cursor.min(n);
    let mut start = cursor;
    while start > 0 && c[start - 1] != '\n' {
        start -= 1;
    }
    let mut end = cursor;
    while end < n && c[end] != '\n' {
        end += 1;
    }
    (start, end)
}

/// Splits `[start, end)` into one sub-range per line, each shrunk to its
/// first and last non-whitespace character.
///
/// This is what keeps a mark off the blank space around the words: an
/// underline or strikethrough applied to an indented line starts at the
/// first word and stops at the last, instead of drawing a line through the
/// indentation and the trailing spaces. Splitting per line matters for a
/// multi-line selection, where the whitespace to skip sits in the middle of
/// the range (the end of one line, the indentation of the next).
///
/// Lines that hold nothing but whitespace contribute no range, so a
/// selection of pure whitespace yields an empty result.
pub fn trimmed_line_spans(text: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let end = end.min(chars.len());
    if start >= end {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut line_start = start;
    let mut i = start;
    // `<= end` so the final (unterminated) segment is flushed by the same code.
    while i <= end {
        let at_break = i == end || chars[i] == '\n';
        if at_break {
            let mut a = line_start;
            let mut b = i;
            while a < b && chars[a].is_whitespace() {
                a += 1;
            }
            while b > a && chars[b - 1].is_whitespace() {
                b -= 1;
            }
            if a < b {
                spans.push((a, b));
            }
            line_start = i + 1;
        }
        i += 1;
    }
    spans
}

// --- Search -----------------------------------------------------------

/// Every occurrence of `pattern` in `text`, as character ranges.
///
/// Plain substring matching, not regular expressions — this is a note
/// editor, and `/` is for finding a word you remember typing. Matching is
/// "smart case", the setting most people give Vim: case-insensitive until
/// the pattern itself contains a capital, which then makes it exact.
pub fn find_matches(text: &str, pattern: &str) -> Vec<(usize, usize)> {
    if pattern.is_empty() {
        return Vec::new();
    }
    let case_sensitive = pattern.chars().any(|c| c.is_uppercase());
    let fold = |s: &str| {
        if case_sensitive {
            s.to_string()
        } else {
            s.to_lowercase()
        }
    };
    // Compare character-by-character so the returned offsets are character
    // offsets, like every other position in this app.
    let hay: Vec<char> = fold(text).chars().collect();
    let needle: Vec<char> = fold(pattern).chars().collect();
    if needle.len() > hay.len() {
        return Vec::new();
    }

    let mut hits = Vec::new();
    let mut i = 0usize;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == needle[..] {
            hits.push((i, i + needle.len()));
            i += needle.len(); // non-overlapping, as Vim's `n` steps
        } else {
            i += 1;
        }
    }
    hits
}

/// The next match strictly after `from`, wrapping to the first.
pub fn match_after(hits: &[(usize, usize)], from: usize) -> Option<(usize, usize)> {
    hits.iter()
        .find(|(s, _)| *s > from)
        .or_else(|| hits.first())
        .copied()
}

/// The previous match strictly before `from`, wrapping to the last.
pub fn match_before(hits: &[(usize, usize)], from: usize) -> Option<(usize, usize)> {
    hits.iter()
        .rev()
        .find(|(s, _)| *s < from)
        .or_else(|| hits.last())
        .copied()
}

// --- URLs -------------------------------------------------------------

/// True for text that looks like something a browser could open. Used both
/// to validate `:link` and to let `gx` work on a bare URL sitting in the
/// text with no link mark on it.
pub fn looks_like_url(token: &str) -> bool {
    let t = token.trim_matches(|c: char| "()[]{}<>,.;:!?\"'".contains(c));
    if t.is_empty() {
        return false;
    }
    t.starts_with("http://")
        || t.starts_with("https://")
        || t.starts_with("mailto:")
        || t.starts_with("www.")
        // A bare domain like example.com/foo: something before a dot,
        // something after it, and no whitespace anywhere.
        || (t.contains('.')
            && !t.starts_with('.')
            && !t.ends_with('.')
            && !t.contains(char::is_whitespace))
}

/// The whitespace-delimited token under the cursor, trimmed of the
/// punctuation that usually surrounds a pasted URL in prose.
pub fn token_at(text: &str, cursor: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let pos = cursor.min(chars.len() - 1);
    if chars[pos].is_whitespace() {
        return None;
    }
    let mut start = pos;
    while start > 0 && !chars[start - 1].is_whitespace() {
        start -= 1;
    }
    let mut end = pos + 1;
    while end < chars.len() && !chars[end].is_whitespace() {
        end += 1;
    }
    let token: String = chars[start..end].iter().collect();
    let trimmed = token
        .trim_matches(|c: char| "()[]{}<>,;:!?\"'".contains(c))
        .trim_end_matches('.')
        .to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Character offset of the start of the last logical line (Vim `G`).
pub fn last_line_start(text: &str) -> usize {
    let n = char_len(text);
    line_bounds(text, n).0
}

/// Length, in characters, of the line containing `char_idx`.
pub fn current_line_len(text: &str, char_idx: usize) -> usize {
    let (row, _) = row_col(text, char_idx);
    text.split('\n')
        .nth(row as usize)
        .map(char_len)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::MarkKind;

    #[test]
    fn insert_and_remove_are_char_index_safe_with_unicode() {
        let mut s = String::from("café");
        insert_char(&mut s, 4, '!'); // after the 'é'
        assert_eq!(s, "café!");
        remove_before(&mut s, 4); // removes the 'é'
        assert_eq!(s, "caf!");
    }

    #[test]
    fn render_lines_splits_on_newlines_and_applies_marks() {
        let text = "hello\nworld";
        let marks = vec![Mark::new(0, 5, MarkKind::Italic)];
        let lines = render_lines(text, &marks, 80);
        assert_eq!(lines.len(), 2);
        // The mark must actually reach the rendered span, not just split.
        assert!(lines[0].spans[0]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::ITALIC));
        assert!(!lines[1].spans[0]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::ITALIC));
    }

    #[test]
    fn long_text_soft_wraps_at_word_boundaries() {
        // 3 rows at width 10: "aaa bbb", "ccc ddd", "eee"
        let lines = render_lines("aaa bbb ccc ddd eee", &[], 10);
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(rendered, vec!["aaa bbb ", "ccc ddd ", "eee"]);
    }

    #[test]
    fn unbroken_word_longer_than_width_hard_wraps() {
        let lines = render_lines("abcdefghij", &[], 4);
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(rendered, vec!["abcd", "efgh", "ij"]);
    }

    /// The renderer and the cursor must agree on every wrap point, or the
    /// cursor drifts away from the text as you type. This checks them
    /// against each other across a whole buffer rather than spot-checking.
    #[test]
    fn cursor_position_agrees_with_rendered_wrapping() {
        let text = "hello world this is a long line\nshort\n\nanother fairly long line here";
        let width = 12;
        let lines = render_lines(text, &[], width);
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();

        for (idx, _) in text.char_indices() {
            let cidx = text[..idx].chars().count();
            let (row, col) = visual_row_col(text, cidx, width);
            let row = row as usize;
            assert!(row < rendered.len(), "row {row} out of range at char {cidx}");
            assert!(
                col as usize <= rendered[row].chars().count(),
                "col {col} past end of row {row} ({:?}) at char {cidx}",
                rendered[row]
            );
            // The character at the cursor must be the one the renderer
            // placed at that row/col (newlines occupy no cell).
            let ch = text.chars().nth(cidx).unwrap();
            if ch != '\n' {
                assert_eq!(
                    rendered[row].chars().nth(col as usize),
                    Some(ch),
                    "char {cidx} ({ch:?}) expected at row {row} col {col} of {:?}",
                    rendered[row]
                );
            }
        }
    }

    #[test]
    fn word_motions_step_between_whitespace_delimited_words() {
        let t = "alpha beta  gamma";
        //       0     6     12
        assert_eq!(next_word_start(t, 0), 6);
        assert_eq!(next_word_start(t, 6), 12);
        assert_eq!(next_word_start(t, 12), t.len()); // past the last word
        assert_eq!(prev_word_start(t, 17), 12);
        assert_eq!(prev_word_start(t, 12), 6);
        assert_eq!(prev_word_start(t, 6), 0);
        assert_eq!(prev_word_start(t, 0), 0);
        assert_eq!(word_end(t, 0), 4); // 'a' of alpha
        assert_eq!(word_end(t, 6), 9); // 'a' of beta
    }

    #[test]
    fn line_helpers_find_bounds_and_blanks() {
        let t = "one\n   two\nthree";
        assert_eq!(line_bounds(t, 0), (0, 3));
        assert_eq!(line_bounds(t, 5), (4, 10));
        assert_eq!(first_non_blank(t, 4), 7); // skips the 3 spaces
        assert_eq!(last_line_start(t), 11);
    }

    /// A literal tab must never reach the terminal, or it jumps to the next
    /// tab stop and every column after it is wrong.
    #[test]
    fn tabs_render_as_a_single_space_so_columns_stay_honest() {
        let lines = render_lines("a\tb\tc", &[], 80);
        let rendered: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "a b c");
        assert!(!rendered.contains('\t'));
        // One column per character, so the cursor maths still lines up.
        assert_eq!(rendered.chars().count(), char_len("a\tb\tc"));
    }

    #[test]
    fn trimmed_spans_skip_indentation_and_trailing_space() {
        let t = "    buy milk   ";
        // Whole line selected -> only "buy milk" (chars 4..12).
        assert_eq!(trimmed_line_spans(t, 0, t.len()), vec![(4, 12)]);
        // Interior space between words stays inside the span.
        assert_eq!(&t[4..12], "buy milk");
    }

    #[test]
    fn trimmed_spans_are_per_line_for_a_multi_line_range() {
        let t = "  first\n\tsecond  \n   third";
        let spans = trimmed_line_spans(t, 0, char_len(t));
        let words: Vec<String> = spans
            .iter()
            .map(|(a, b)| t.chars().skip(*a).take(b - a).collect())
            .collect();
        assert_eq!(words, vec!["first", "second", "third"]);
    }

    #[test]
    fn trimmed_spans_ignore_blank_and_whitespace_only_lines() {
        let t = "one\n\n    \ntwo";
        let spans = trimmed_line_spans(t, 0, char_len(t));
        let words: Vec<String> = spans
            .iter()
            .map(|(a, b)| t.chars().skip(*a).take(b - a).collect())
            .collect();
        assert_eq!(words, vec!["one", "two"]);
        // Pure whitespace yields nothing at all.
        assert!(trimmed_line_spans("   ", 0, 3).is_empty());
        assert!(trimmed_line_spans("", 0, 0).is_empty());
    }

    #[test]
    fn trimmed_spans_respect_a_partial_range() {
        let t = "alpha beta gamma";
        // Selecting " beta " mid-line trims to just "beta".
        assert_eq!(trimmed_line_spans(t, 5, 11), vec![(6, 10)]);
        assert_eq!(&t[6..10], "beta");
    }

    /// Selecting a link used to make it invisible: the link's blue
    /// foreground sat on a blue selection background. Whatever a mark
    /// colours the text, a selected span must stay readable.
    #[test]
    fn selected_text_is_readable_over_any_mark() {
        let text = "click here now";
        for kind in [
            MarkKind::Link,
            MarkKind::Code,
            MarkKind::Heading,
            MarkKind::Highlight,
            MarkKind::Bold,
        ] {
            let marks = vec![Mark::new(6, 10, kind)];
            let lines = render_lines_sel(text, &marks, 80, Some((0, 14)), &[]);
            for span in lines.iter().flat_map(|l| l.spans.iter()) {
                assert_ne!(
                    span.style.fg, span.style.bg,
                    "{kind:?} rendered {:?} with fg == bg",
                    span.content
                );
                assert_eq!(span.style.bg, Some(SELECTION_BG), "{kind:?}");
                assert_eq!(span.style.fg, Some(SELECTION_FG), "{kind:?}");
            }
        }
    }

    /// The same hazard applies to search hits sitting under a coloured mark.
    /// Only the highlighted spans are checked: unstyled text legitimately
    /// has neither colour set.
    #[test]
    fn search_hits_are_readable_over_any_mark() {
        let text = "click here now";
        for kind in [
            MarkKind::Link,
            MarkKind::Code,
            MarkKind::Heading,
            MarkKind::Highlight,
        ] {
            let marks = vec![Mark::new(6, 10, kind)];
            let lines = render_lines_sel(text, &marks, 80, None, &[(6, 10)]);
            let hit = lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .find(|s| s.content == "here")
                .unwrap_or_else(|| panic!("{kind:?}: no span for the match"));
            assert_eq!(hit.style.bg, Some(SEARCH_BG), "{kind:?}");
            assert_ne!(hit.style.fg, hit.style.bg, "{kind:?} left fg == bg");
        }
    }

    /// The rows the cursor moves through must be the rows on screen: one
    /// long line shown as three rows is three rows to `j`/`k` and to `V`.
    #[test]
    fn visual_rows_follow_what_is_rendered() {
        let text = "aaa bbb ccc ddd\nshort";
        let width = 8;
        // Rendered: "aaa bbb ", "ccc ddd", "short"
        let rendered: Vec<String> = render_lines(text, &[], width)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        let rows = visual_rows(text, width);
        assert_eq!(rows.len(), rendered.len(), "row count vs rendered rows");
        for (i, (start, end)) in rows.iter().enumerate() {
            let slice: String = text.chars().skip(*start).take(end - start).collect();
            assert_eq!(slice, rendered[i], "row {i}");
        }
    }

    #[test]
    fn vertical_movement_steps_one_visual_row_at_a_time() {
        let text = "aaa bbb ccc ddd\nshort";
        let width = 8;
        // From the first row, down lands inside the *same* logical line.
        let down1 = visual_move(text, 0, width, 1);
        assert_eq!(visual_row_col(text, down1, width).0, 1);
        let down2 = visual_move(text, down1, width, 1);
        assert_eq!(visual_row_col(text, down2, width).0, 2);
        // And back up again, row by row.
        assert_eq!(visual_move(text, down2, width, -1), down1);
        assert_eq!(visual_move(text, down1, width, -1), 0);
        // Clamped at both ends instead of refusing to move.
        assert_eq!(visual_move(text, 0, width, -5), 0);
        assert_eq!(
            visual_row_col(text, visual_move(text, 0, width, 99), width).0,
            2
        );
    }

    #[test]
    fn v_selects_one_visual_row_not_the_whole_wrapped_line() {
        let text = "aaa bbb ccc ddd";
        let width = 8;
        // Cursor on the first row selects only that row...
        assert_eq!(visual_row_bounds(text, 1, width), (0, 8));
        // ...and on the second row, only the rest.
        assert_eq!(visual_row_bounds(text, 9, width), (8, 15));
        // With no wrapping it is simply the whole line.
        assert_eq!(visual_row_bounds(text, 1, 80), (0, 15));
    }

    #[test]
    fn search_is_smart_case_and_non_overlapping() {
        let t = "the Cat sat on the cat mat";
        // Lower-case pattern matches either case.
        let hits = find_matches(t, "cat");
        assert_eq!(hits.len(), 2);
        // A capital in the pattern makes it exact.
        assert_eq!(find_matches(t, "Cat"), vec![(4, 7)]);
        // Overlapping repeats step past each match, like Vim's `n`.
        assert_eq!(find_matches("aaaa", "aa"), vec![(0, 2), (2, 4)]);
        assert!(find_matches(t, "dog").is_empty());
        assert!(find_matches(t, "").is_empty());
    }

    #[test]
    fn search_stepping_wraps_in_both_directions() {
        let hits = vec![(2, 5), (10, 13), (20, 23)];
        assert_eq!(match_after(&hits, 0), Some((2, 5)));
        assert_eq!(match_after(&hits, 2), Some((10, 13)));
        // Past the last match, wrap to the first.
        assert_eq!(match_after(&hits, 25), Some((2, 5)));
        assert_eq!(match_before(&hits, 20), Some((10, 13)));
        // Before the first, wrap to the last.
        assert_eq!(match_before(&hits, 0), Some((20, 23)));
        assert_eq!(match_after(&[], 0), None);
    }

    #[test]
    fn url_detection_and_token_under_cursor() {
        assert!(looks_like_url("https://example.com"));
        assert!(looks_like_url("www.example.com"));
        assert!(looks_like_url("example.com/path"));
        assert!(!looks_like_url("hello"));
        assert!(!looks_like_url("two words"));

        let t = "see https://rust-lang.org, it is good";
        // Cursor inside the URL returns it with the comma stripped.
        assert_eq!(token_at(t, 10).as_deref(), Some("https://rust-lang.org"));
        // Cursor on whitespace has no token.
        assert_eq!(token_at(t, 3), None);
        assert_eq!(token_at(t, 0).as_deref(), Some("see"));
    }

    #[test]
    fn remove_range_is_char_indexed() {
        let mut s = String::from("héllo wörld");
        remove_range(&mut s, 0, 6);
        assert_eq!(s, "wörld");
    }

    #[test]
    fn selection_is_highlighted_across_a_wrap() {
        let lines = render_lines_sel("aaa bbb ccc", &[], 4, Some((2, 9)), &[]);
        let highlighted: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.style.bg == Some(SELECTION_BG))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(highlighted, "a bbb c");
    }
}
