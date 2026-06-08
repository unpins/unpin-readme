//! Render markdown to styled terminal text and page it.
//!
//! The pager is built on termimad's `MadView`, which owns the markdown and its
//! area: every redraw re-runs the formatter at the current width. So on a
//! terminal resize we re-render the markdown to the new width — true reflow,
//! the way `unpin-man`'s built-in pager re-runs mandoc on SIGWINCH — instead of
//! re-wrapping text that was already wrapped to the old width. When stdout is
//! not a tty we emit one plain, width-wrapped render so `unpin readme pkg | …`
//! keeps working.

use std::io::{self, IsTerminal, Write};

use termimad::crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Print, SetAttribute},
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use termimad::{Area, MadSkin, MadView};

/// Render `md` and show it. On a tty, page it with the built-in reflowing
/// pager; otherwise just print the rendered text wrapped to the terminal width.
pub fn page(md: &str) {
    if !io::stdout().is_terminal() {
        print_plain(md);
        return;
    }
    // On any pager/terminal failure, fall back to a plain print so the README
    // is still shown (e.g. a terminal that rejects raw mode).
    if run_pager(md).is_err() {
        print_plain(md);
    }
}

/// One plain render wrapped to the current width (80 when unknown). Used when
/// stdout is redirected and as the non-interactive fallback.
fn print_plain(md: &str) {
    let width = terminal::size().map(|(w, _)| w as usize).unwrap_or(80);
    print!("{}", MadSkin::default().text(md, Some(width)));
}

/// Enter the alternate screen + raw mode, run the pager loop, and always
/// restore the terminal afterwards — even if the loop errors.
fn run_pager(md: &str) -> io::Result<()> {
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    queue!(out, EnterAlternateScreen, cursor::Hide, Clear(ClearType::All))?;
    out.flush()?;

    let res = pager_loop(&mut out, md);

    let _ = execute!(out, cursor::Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    res
}

/// The pager event loop: draw, block for an event, scroll/resize/quit, repeat.
fn pager_loop<W: Write>(out: &mut W, md: &str) -> io::Result<()> {
    let (mut cols, mut rows) = terminal::size().unwrap_or((80, 24));
    let mut view = MadView::from(md.to_owned(), content_area(cols, rows), MadSkin::default());

    loop {
        // `MadView::write_on` reformats the markdown for its area every call,
        // so this redraws (and reflows) at the live width. It fills the whole
        // area, leaving no stale cells when the terminal shrank.
        view.write_on(out).map_err(|e| io::Error::other(e.to_string()))?;
        draw_status(out, cols, rows)?;
        out.flush()?;

        match event::read()? {
            Event::Resize(c, r) => {
                cols = c;
                rows = r;
                view.resize(&content_area(cols, rows));
                queue!(out, Clear(ClearType::All))?;
            }
            // Ignore key-release events (kitty protocol); act on press/repeat.
            // `handle_key` returns false to quit (only called for non-release).
            Event::Key(key)
                if key.kind != KeyEventKind::Release
                    && !handle_key(&mut view, key, rows) =>
            {
                return Ok(());
            }
            _ => {}
        }
    }
}

/// The content area: the full terminal minus the bottom status row.
fn content_area(cols: u16, rows: u16) -> Area {
    let height = if rows >= 2 { rows - 1 } else { rows.max(1) };
    Area::new(0, 0, cols.max(1), height)
}

/// Apply a key to the view. Returns `false` to quit, `true` to keep paging.
fn handle_key(view: &mut MadView, key: KeyEvent, rows: u16) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let half = i32::from(rows / 2).max(1);
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return false,
        KeyCode::Char('c') if ctrl => return false,
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Enter => view.try_scroll_lines(1),
        KeyCode::Up | KeyCode::Char('k') => view.try_scroll_lines(-1),
        KeyCode::Char('d') if ctrl => view.try_scroll_lines(half),
        KeyCode::Char('u') if ctrl => view.try_scroll_lines(-half),
        KeyCode::PageDown | KeyCode::Char(' ') | KeyCode::Char('f') => view.try_scroll_pages(1),
        KeyCode::PageUp | KeyCode::Char('b') => view.try_scroll_pages(-1),
        KeyCode::Home | KeyCode::Char('g') => view.scroll = 0,
        KeyCode::End | KeyCode::Char('G') => view.try_scroll_lines(i32::MAX),
        _ => {}
    }
    true
}

/// A reverse-video help line across the bottom row. Skipped on a 1-row terminal.
fn draw_status<W: Write>(out: &mut W, cols: u16, rows: u16) -> io::Result<()> {
    if rows < 2 {
        return Ok(());
    }
    let width = cols.max(1) as usize;
    let hint = " q quit · ↑/↓ j/k scroll · Space/b page · g/G top/bottom ";
    // Set Reverse first, then erase-to-end-of-line: the terminal fills the
    // cleared cells with the current rendition, so the whole row is painted
    // reverse in one move — no manual space-padding, and no char-vs-column
    // width guesswork. The hint is printed on top of the left of that row;
    // `take(width)` is just a safety cap so it can't wrap a narrow terminal.
    let line: String = hint.chars().take(width).collect();
    queue!(
        out,
        cursor::MoveTo(0, rows - 1),
        SetAttribute(Attribute::Reverse),
        Clear(ClearType::UntilNewLine),
        Print(line),
        SetAttribute(Attribute::Reset),
    )?;
    Ok(())
}
