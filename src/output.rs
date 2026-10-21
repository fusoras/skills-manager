//! Terminal output helpers: ANSI styling, aligned tables and broken-pipe detection.
//!
//! Colors are enabled only when stdout is a terminal and `NO_COLOR` is unset, so
//! piped output stays plain and stable for scripts and tests.

use std::io::{IsTerminal, Write};

/// ANSI: reset all attributes.
pub const RESET: &str = "\x1b[0m";
/// ANSI: bold.
pub const BOLD: &str = "\x1b[1m";
/// ANSI: red (failures).
pub const RED: &str = "\x1b[31m";
/// ANSI: green (success).
pub const GREEN: &str = "\x1b[32m";
/// ANSI: yellow (attention).
pub const YELLOW: &str = "\x1b[33m";
/// ANSI: cyan (identifiers).
pub const CYAN: &str = "\x1b[36m";
/// ANSI: bright black / gray (dimmed descriptions).
pub const GRAY: &str = "\x1b[90m";

/// Decides whether ANSI codes are emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    enabled: bool,
}

impl Style {
    /// Colors on when stdout is a terminal and `NO_COLOR` is unset or empty.
    pub fn auto() -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        Self {
            enabled: std::io::stdout().is_terminal() && !no_color,
        }
    }

    /// Whether colors and ANSI styling are enabled.
    pub fn is_enabled(self) -> bool {
        self.enabled
    }

    /// Colors off, for tests and non-tty writers.
    #[cfg(test)]
    pub fn plain() -> Self {
        Self { enabled: false }
    }

    /// Colors on, for tests.
    #[cfg(test)]
    pub fn colored() -> Self {
        Self { enabled: true }
    }

    /// Wrap `text` in `code` .. `RESET` when colors are enabled.
    pub fn paint(self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("{code}{text}{RESET}")
        } else {
            text.to_owned()
        }
    }

    /// Green success marker.
    pub fn ok(self, text: &str) -> String {
        self.paint(GREEN, text)
    }

    /// Yellow attention marker.
    pub fn warn(self, text: &str) -> String {
        self.paint(YELLOW, text)
    }

    /// Red failure marker.
    pub fn fail(self, text: &str) -> String {
        self.paint(RED, text)
    }

    /// Cyan identifier marker.
    pub fn ident(self, text: &str) -> String {
        self.paint(CYAN, text)
    }

    /// Gray dimmed marker for descriptions.
    pub fn gray(self, text: &str) -> String {
        self.paint(GRAY, text)
    }
}

/// Query the current terminal width in columns.
///
/// Priority:
/// 1. `COLUMNS` environment variable (if set and valid positive integer).
/// 2. `terminal_size::terminal_size()`.
pub fn terminal_width() -> Option<usize> {
    let env_cols = std::env::var("COLUMNS").ok();
    terminal_width_from(env_cols.as_deref())
}

/// Helper that checks `COLUMNS` first, falling back to `terminal_size`.
pub fn terminal_width_from(env_columns: Option<&str>) -> Option<usize> {
    if let Some(val) = env_columns
        && let Ok(cols) = val.trim().parse::<usize>()
        && cols > 0
    {
        return Some(cols);
    }
    terminal_size::terminal_size().map(|(terminal_size::Width(w), _)| w as usize)
}

/// Writer wrapper that renders styled lines and tables.
pub struct Printer {
    out: Box<dyn Write>,
    style: Style,
}

impl Printer {
    /// Printer over the process stdout, styled for the current terminal.
    pub fn stdout() -> Self {
        Self {
            out: Box::new(std::io::stdout().lock()),
            style: Style::auto(),
        }
    }

    /// Printer over an arbitrary writer (used by tests).
    #[cfg(test)]
    pub fn with_writer(out: Box<dyn Write>, style: Style) -> Self {
        Self { out, style }
    }

    /// Style in use.
    pub fn style(&self) -> Style {
        self.style
    }

    /// Write one line.
    pub fn line(&mut self, text: &str) -> std::io::Result<()> {
        writeln!(self.out, "{text}")
    }

    /// Write an empty line.
    #[cfg(test)]
    pub fn blank(&mut self) -> std::io::Result<()> {
        writeln!(self.out)
    }

    /// Write an aligned table with a bold header row.
    pub fn table(&mut self, headers: &[&str], rows: &[Vec<String>]) -> std::io::Result<()> {
        write!(self.out, "{}", render_table(self.style, headers, rows))
    }

    /// Flush the underlying writer.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

/// Render `headers` and `rows` as a left-aligned table with a bold header.
///
/// Padding is computed from the visible width of each cell, so cells may carry
/// ANSI codes without breaking the alignment. Trailing whitespace is trimmed.
pub fn render_table(style: Style, headers: &[&str], rows: &[Vec<String>]) -> String {
    let columns = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|head| visible_width(head)).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate().take(columns) {
            widths[index] = widths[index].max(visible_width(cell));
        }
    }

    let mut rendered = String::new();
    let head: Vec<String> = headers.iter().map(|text| style.paint(BOLD, text)).collect();
    push_row(&mut rendered, &head, &widths);
    for row in rows {
        let normalized: Vec<String> = (0..columns)
            .map(|index| row.get(index).cloned().unwrap_or_default())
            .collect();
        push_row(&mut rendered, &normalized, &widths);
    }
    rendered
}

fn push_row(rendered: &mut String, cells: &[String], widths: &[usize]) {
    let last = cells.len().saturating_sub(1);
    let mut line = String::new();
    for (index, cell) in cells.iter().enumerate() {
        line.push_str(cell);
        if index < last {
            let padding = widths[index].saturating_sub(visible_width(cell)) + 2;
            line.extend(std::iter::repeat_n(' ', padding));
        }
    }
    rendered.push_str(line.trim_end());
    rendered.push('\n');
}

/// Visible width of `text`, ignoring ANSI SGR escape sequences.
pub fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;
    for character in text.chars() {
        if in_escape {
            if character == 'm' {
                in_escape = false;
            }
        } else if character == '\x1b' {
            in_escape = true;
        } else {
            width += 1;
        }
    }
    width
}

/// `true` when `error` was caused by a closed stdout (for example `skm | head`).
pub fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::BrokenPipe)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_width_ignores_ansi_sequences() {
        assert_eq!(visible_width("\x1b[1mOK\x1b[0m"), 2);
        assert_eq!(visible_width("OK"), 2);
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn render_table_pads_columns_from_visible_width() {
        let rows = vec![
            vec!["a".to_owned(), "long-value".to_owned()],
            vec!["bbbb".to_owned(), "x".to_owned()],
        ];
        let expected = "NAME  VALUE\n\
                        a     long-value\n\
                        bbbb  x\n";
        assert_eq!(
            render_table(Style::plain(), &["NAME", "VALUE"], &rows),
            expected
        );
    }

    #[test]
    fn render_table_ignores_color_codes_when_aligning() {
        let styled = Style { enabled: true };
        let rows = vec![vec![styled.paint(GREEN, "OK"), "x".to_owned()]];
        let table = render_table(styled, &["STATE", "P"], &rows);
        let expected = format!(
            "{}  {}\n{}     x\n",
            styled.paint(BOLD, "STATE"),
            styled.paint(BOLD, "P"),
            styled.paint(GREEN, "OK")
        );
        assert_eq!(table, expected);
    }

    #[test]
    fn render_table_trims_trailing_whitespace() {
        let rows = vec![vec!["a".to_owned(), String::new()]];
        assert_eq!(
            render_table(Style::plain(), &["N", "M"], &rows),
            "N  M\na\n"
        );
    }

    #[test]
    fn style_plain_never_paints() {
        assert_eq!(Style::plain().ok("OK"), "OK");
        assert_eq!(Style::plain().fail("BROKEN"), "BROKEN");
        assert_eq!(Style::plain().gray("dimmed"), "dimmed");
    }

    #[test]
    fn style_enabled_paints_with_reset() {
        let style = Style { enabled: true };
        assert_eq!(style.ok("OK"), "\x1b[32mOK\x1b[0m");
        assert_eq!(style.warn("w"), "\x1b[33mw\x1b[0m");
        assert_eq!(style.fail("e"), "\x1b[31me\x1b[0m");
        assert_eq!(style.ident("id"), "\x1b[36mid\x1b[0m");
        assert_eq!(style.gray("dim"), "\x1b[90mdim\x1b[0m");
        assert_eq!(style.paint(BOLD, "b"), "\x1b[1mb\x1b[0m");
    }

    #[test]
    fn terminal_width_reads_columns_env() {
        assert_eq!(terminal_width_from(Some("100")), Some(100));
        assert_eq!(terminal_width_from(Some("42")), Some(42));
        assert_eq!(
            terminal_width_from(Some("0")),
            terminal_size::terminal_size().map(|(w, _)| w.0 as usize)
        );
        assert_eq!(
            terminal_width_from(Some("invalid")),
            terminal_size::terminal_size().map(|(w, _)| w.0 as usize)
        );
    }

    /// Writer that keeps everything written to it, shareable with the test.
    #[derive(Debug, Clone, Default)]
    struct SharedWriter(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

    impl std::io::Write for SharedWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn printer_writes_lines_and_tables_to_the_writer() {
        let sink = SharedWriter::default();
        {
            let mut printer = Printer::with_writer(Box::new(sink.clone()), Style::plain());
            printer.line("hello").unwrap();
            printer.blank().unwrap();
            printer.table(&["A"], &[vec!["b".to_owned()]]).unwrap();
            printer.flush().unwrap();
        }
        let text = String::from_utf8(sink.0.borrow().clone()).unwrap();
        assert_eq!(text, "hello\n\nA\nb\n");
    }

    #[test]
    fn is_broken_pipe_detects_closed_stdout() {
        let broken = anyhow::Error::from(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "pipe closed",
        ))
        .context("writing report");
        assert!(is_broken_pipe(&broken));
        let other = anyhow::Error::from(std::io::Error::new(std::io::ErrorKind::NotFound, "nope"));
        assert!(!is_broken_pipe(&other));
    }
}
