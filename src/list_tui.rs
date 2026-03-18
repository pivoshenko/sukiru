use std::cmp::{max, min};
use std::io::{stdout, Stdout, Write};

use crossterm::cursor::{
    Hide, MoveRight, MoveToColumn, MoveToNextLine, MoveUp, RestorePosition, SavePosition, Show,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::queue;
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use crossterm::{execute, terminal::size};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::banner::{banner_lines, banner_width};
use crate::error::Result;
use crate::model::InstalledSkill;

pub fn browse(items: &[InstalledSkill]) -> Result<()> {
    let mut guard = TerminalGuard::enter()?;
    let mut stdout = stdout();
    let mut state = ListState::default();

    loop {
        draw(&mut stdout, items, &mut state, &mut guard)?;
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up | KeyCode::Char('k') => state.move_by(-1, items.len()),
                KeyCode::Down | KeyCode::Char('j') => state.move_by(1, items.len()),
                KeyCode::PageUp => state.page_up(items.len()),
                KeyCode::PageDown => state.page_down(items.len()),
                KeyCode::Home | KeyCode::Char('g') => state.jump_to(0, items.len()),
                KeyCode::End | KeyCode::Char('G') => {
                    state.jump_to(items.len().saturating_sub(1), items.len())
                }
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }

    Ok(())
}

#[derive(Default)]
struct ListState {
    selected: usize,
    scroll: usize,
    last_page_size: usize,
}

impl ListState {
    fn move_by(&mut self, delta: isize, len: usize) {
        if len == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        let next = self.selected as isize + delta;
        self.selected = next.clamp(0, len as isize - 1) as usize;
    }

    fn jump_to(&mut self, index: usize, len: usize) {
        if len == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = min(index, len - 1);
    }

    fn page_up(&mut self, len: usize) {
        let step = max(1, self.last_page_size.saturating_sub(1));
        self.move_by(-(step as isize), len);
    }

    fn page_down(&mut self, len: usize) {
        let step = max(1, self.last_page_size.saturating_sub(1));
        self.move_by(step as isize, len);
    }

    fn keep_visible(&mut self, visible_rows: usize, len: usize) {
        self.last_page_size = visible_rows;
        if len == 0 || visible_rows == 0 {
            self.scroll = 0;
            return;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        let max_scroll = len.saturating_sub(visible_rows);
        if self.selected >= self.scroll + visible_rows {
            self.scroll = self.selected + 1 - visible_rows;
        }
        self.scroll = min(self.scroll, max_scroll);
    }
}

struct TerminalGuard {
    height: u16,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        let (_, term_height) = size()?;
        let height = term_height.clamp(10, 26);
        enable_raw_mode()?;
        execute!(stdout(), Hide)?;

        if height > 1 {
            for _ in 1..height {
                println!();
            }
            execute!(stdout(), MoveUp(height - 1))?;
        }
        execute!(stdout(), SavePosition)?;

        Ok(Self { height })
    }

    fn refresh_size(&mut self) -> Result<()> {
        let (_, term_height) = size()?;
        self.height = term_height.clamp(10, 26);
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            stdout(),
            RestorePosition,
            MoveToNextLine(self.height),
            Clear(ClearType::CurrentLine),
            Show
        );
        println!();
    }
}

fn draw(
    stdout: &mut Stdout,
    items: &[InstalledSkill],
    state: &mut ListState,
    guard: &mut TerminalGuard,
) -> Result<()> {
    guard.refresh_size()?;
    let (width, _) = size()?;
    let width = width as usize;
    let panel_height = guard.height as usize;
    let colors = Colors::active();

    clear_panel(stdout, guard.height)?;

    if width < 72 || panel_height < 8 {
        draw_small_terminal(stdout, width, panel_height, &colors)?;
        stdout.flush()?;
        return Ok(());
    }

    let mut row = 0usize;
    let show_ascii_banner = width >= banner_width() && panel_height >= 16;
    if show_ascii_banner {
        row += draw_banner(stdout, width, row, &colors)?;
    } else {
        row += draw_compact_banner(stdout, width, row, &colors)?;
    }
    row = draw_header(stdout, width, row, items.len(), &colors)?;

    let footer_height = 2usize;
    let content_top = row;
    let content_height = panel_height.saturating_sub(content_top + footer_height);
    if content_height < 4 {
        clear_panel(stdout, guard.height)?;
        draw_small_terminal(stdout, width, panel_height, &colors)?;
        stdout.flush()?;
        return Ok(());
    }

    let two_pane = content_height >= 8;
    let side_by_side = two_pane && width >= 80;
    if two_pane && !side_by_side {
        let list_height = max(5, content_height / 2);
        state.keep_visible(list_height.saturating_sub(2), items.len());
        draw_list_pane(
            stdout,
            0,
            content_top,
            width,
            list_height,
            items,
            state,
            &colors,
        )?;
        draw_detail_pane(
            stdout,
            0,
            content_top + list_height,
            width,
            content_height.saturating_sub(list_height),
            items.get(state.selected),
            &colors,
        )?;
    } else if side_by_side {
        let list_width = min(max(34, width / 3), 46);
        let detail_width = width.saturating_sub(list_width + 1);
        state.keep_visible(content_height.saturating_sub(2), items.len());
        draw_list_pane(
            stdout,
            0,
            content_top,
            list_width,
            content_height,
            items,
            state,
            &colors,
        )?;
        draw_detail_pane(
            stdout,
            list_width + 1,
            content_top,
            detail_width,
            content_height,
            items.get(state.selected),
            &colors,
        )?;
    } else {
        state.keep_visible(content_height.saturating_sub(2), items.len());
        draw_list_pane(
            stdout,
            0,
            content_top,
            width,
            content_height,
            items,
            state,
            &colors,
        )?;
    }

    draw_footer(
        stdout,
        width,
        panel_height.saturating_sub(2),
        items.get(state.selected),
        two_pane,
        &colors,
    )?;
    stdout.flush()?;
    Ok(())
}

fn move_to(stdout: &mut Stdout, left: usize, top: usize) -> Result<()> {
    queue!(stdout, RestorePosition, MoveToColumn(0))?;
    if top > 0 {
        queue!(stdout, MoveToNextLine(top as u16))?;
    }
    if left > 0 {
        queue!(stdout, MoveRight(left as u16))?;
    }
    Ok(())
}

fn clear_panel(stdout: &mut Stdout, height: u16) -> Result<()> {
    for offset in 0..height {
        move_to(stdout, 0, offset as usize)?;
        queue!(stdout, Clear(ClearType::CurrentLine))?;
    }
    Ok(())
}

fn draw_small_terminal(
    stdout: &mut Stdout,
    width: usize,
    height: usize,
    colors: &Colors,
) -> Result<()> {
    let lines = [
        "kasetto list",
        "",
        "Terminal too small for the skill browser.",
        "Resize the window to at least 72x20.",
        "Press q to exit.",
    ];
    for (index, line) in lines.iter().enumerate() {
        if index >= height {
            break;
        }
        write_line(stdout, 0, index, width, line, colors, Style::Title)?;
    }
    Ok(())
}

fn draw_banner(stdout: &mut Stdout, width: usize, top: usize, colors: &Colors) -> Result<usize> {
    if width >= banner_width() {
        for (offset, line) in banner_lines().iter().enumerate() {
            move_to(stdout, 0, top + offset)?;
            queue!(
                stdout,
                SetForegroundColor(colors.banner),
                Print(line),
                ResetColor
            )?;
        }
        Ok(banner_lines().len() + 1)
    } else {
        write_line(stdout, 0, top, width, "kasetto", colors, Style::Title)?;
        Ok(2)
    }
}

fn draw_compact_banner(
    stdout: &mut Stdout,
    width: usize,
    top: usize,
    colors: &Colors,
) -> Result<usize> {
    write_line(
        stdout,
        0,
        top,
        width,
        "kasetto | skill browser",
        colors,
        Style::Title,
    )?;
    Ok(1)
}

fn draw_header(
    stdout: &mut Stdout,
    width: usize,
    top: usize,
    count: usize,
    colors: &Colors,
) -> Result<usize> {
    let summary = format!("{} installed  |  Navigate with ↑ ↓ j k PgUp PgDn", count);
    write_line(stdout, 0, top, width, &summary, colors, Style::Muted)?;
    Ok(top + 1)
}

fn draw_list_pane(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    items: &[InstalledSkill],
    state: &ListState,
    colors: &Colors,
) -> Result<()> {
    if width < 10 || height < 4 {
        return Ok(());
    }

    draw_box(stdout, left, top, width, height, "Installed Skills", colors)?;
    let inner_width = width.saturating_sub(2);
    let visible_rows = height.saturating_sub(2);
    let start = state.scroll;
    let end = min(start + visible_rows, items.len());

    for row in 0..visible_rows {
        let y = top + 1 + row;
        let item_index = start + row;
        if item_index >= end {
            write_fill(stdout, left + 1, y, inner_width, colors.background)?;
            continue;
        }

        let item = &items[item_index];
        let label = truncate_width(&item.name, inner_width);

        move_to(stdout, left + 1, y)?;
        queue!(
            stdout,
            SetBackgroundColor(if item_index == state.selected {
                colors.selection_bg
            } else {
                colors.background
            }),
            SetForegroundColor(if item_index == state.selected {
                colors.selection_fg
            } else {
                colors.text
            }),
            Print(pad_width(&label, inner_width)),
            ResetColor
        )?;
    }

    Ok(())
}

fn draw_detail_pane(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    item: Option<&InstalledSkill>,
    colors: &Colors,
) -> Result<()> {
    if width < 10 || height < 4 {
        return Ok(());
    }

    draw_box(stdout, left, top, width, height, "Details", colors)?;
    let inner_left = left + 1;
    let inner_top = top + 1;
    let inner_width = width.saturating_sub(2);
    let inner_height = height.saturating_sub(2);

    let Some(item) = item else {
        return Ok(());
    };

    let mut lines = Vec::new();
    let description = if item.description.is_empty() {
        "No description."
    } else {
        item.description.as_str()
    };
    lines.push(Line::label_value("Name", &item.name));
    lines.push(Line::label_value(
        "Updated",
        &format!("{} ({})", item.updated_ago, item.updated_at),
    ));
    lines.push(Line::label_value("Description", description));

    let wrapped = wrap_lines(&lines, inner_width);
    for row in 0..inner_height {
        let y = inner_top + row;
        if let Some(line) = wrapped.get(row) {
            write_styled_line(stdout, inner_left, y, inner_width, line, colors)?;
        } else {
            write_fill(stdout, inner_left, y, inner_width, colors.background)?;
        }
    }

    Ok(())
}

fn draw_footer(
    stdout: &mut Stdout,
    width: usize,
    top: usize,
    item: Option<&InstalledSkill>,
    show_detail_help: bool,
    colors: &Colors,
) -> Result<()> {
    write_line(
        stdout,
        0,
        top,
        width,
        "q quit   ↑/↓ or j/k move   PgUp/PgDn page   g/G jump",
        colors,
        Style::Muted,
    )?;
    let second_line = if show_detail_help {
        "Use --json for machine-readable output.".to_string()
    } else if let Some(item) = item {
        format!("Selected: {}  |  Updated {}", item.name, item.updated_ago)
    } else {
        "Use --json for machine-readable output.".to_string()
    };
    write_line(
        stdout,
        0,
        top + 1,
        width,
        &second_line,
        colors,
        Style::Muted,
    )?;
    Ok(())
}

fn draw_box(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    title: &str,
    colors: &Colors,
) -> Result<()> {
    move_to(stdout, left, top)?;
    let horizontal = "─".repeat(width.saturating_sub(2));
    queue!(
        stdout,
        SetForegroundColor(colors.border),
        Print("┌"),
        Print(horizontal.clone()),
        Print("┐"),
        ResetColor
    )?;

    for offset in 1..height.saturating_sub(1) {
        move_to(stdout, left, top + offset)?;
        queue!(
            stdout,
            SetForegroundColor(colors.border),
            Print("│"),
            SetBackgroundColor(colors.background),
            Print(" ".repeat(width.saturating_sub(2))),
            ResetColor,
            SetForegroundColor(colors.border),
            Print("│"),
            ResetColor
        )?;
    }

    if height >= 2 {
        move_to(stdout, left, top + height - 1)?;
        queue!(
            stdout,
            SetForegroundColor(colors.border),
            Print("└"),
            Print("─".repeat(width.saturating_sub(2))),
            Print("┘"),
            ResetColor
        )?;
    }

    let title_text = truncate_width(&format!(" {} ", title), width.saturating_sub(4));
    move_to(stdout, left + 2, top)?;
    queue!(
        stdout,
        SetForegroundColor(colors.accent),
        Print(title_text),
        ResetColor
    )?;
    Ok(())
}

fn write_fill(stdout: &mut Stdout, left: usize, top: usize, width: usize, bg: Color) -> Result<()> {
    move_to(stdout, left, top)?;
    queue!(
        stdout,
        SetBackgroundColor(bg),
        Print(" ".repeat(width)),
        ResetColor
    )?;
    Ok(())
}

fn write_line(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    text: &str,
    colors: &Colors,
    style: Style,
) -> Result<()> {
    let line = StyledLine::new(style, truncate_width(text, width));
    write_styled_line(stdout, left, top, width, &line, colors)
}

fn write_styled_line(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    line: &StyledLine,
    colors: &Colors,
) -> Result<()> {
    move_to(stdout, left, top)?;
    match line.style {
        Style::Title => queue!(
            stdout,
            SetForegroundColor(colors.accent),
            SetAttribute(Attribute::Bold)
        )?,
        Style::Muted => queue!(stdout, SetForegroundColor(colors.muted))?,
        Style::Value => queue!(stdout, SetForegroundColor(colors.text))?,
    }
    queue!(
        stdout,
        SetBackgroundColor(colors.background),
        Print(pad_width(&line.text, width)),
        ResetColor,
        SetAttribute(Attribute::Reset)
    )?;
    Ok(())
}

fn wrap_lines(lines: &[Line], width: usize) -> Vec<StyledLine> {
    let mut wrapped = Vec::new();
    for line in lines {
        match line {
            Line::LabelValue(label, value) => {
                wrapped.push(StyledLine::new(Style::Muted, format!("{label}:")));
                wrapped.extend(wrap_text(value, width, Style::Value));
                wrapped.push(StyledLine::new(Style::Value, String::new()));
            }
        }
    }
    while matches!(wrapped.last(), Some(StyledLine { text, .. }) if text.is_empty()) {
        wrapped.pop();
    }
    wrapped
}

fn wrap_text(text: &str, width: usize, style: Style) -> Vec<StyledLine> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for word in text.split_whitespace() {
        let word_width = UnicodeWidthStr::width(word);
        let spacer = usize::from(!current.is_empty());
        if current_width + spacer + word_width > width && !current.is_empty() {
            lines.push(StyledLine::new(style, current));
            current = word.to_string();
            current_width = word_width;
        } else {
            if !current.is_empty() {
                current.push(' ');
                current_width += 1;
            }
            current.push_str(word);
            current_width += word_width;
        }
    }

    if current.is_empty() {
        lines.push(StyledLine::new(style, String::new()));
    } else {
        lines.push(StyledLine::new(style, current));
    }
    lines
}

fn truncate_width(text: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "…".to_string();
    }

    let mut out = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width + 1 > max_width {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out.push('…');
    out
}

fn pad_width(text: &str, width: usize) -> String {
    let actual = UnicodeWidthStr::width(text);
    if actual >= width {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(width - actual))
    }
}

#[derive(Clone, Copy)]
enum Style {
    Title,
    Muted,
    Value,
}

struct StyledLine {
    style: Style,
    text: String,
}

impl StyledLine {
    fn new(style: Style, text: String) -> Self {
        Self { style, text }
    }
}

enum Line {
    LabelValue(String, String),
}

impl Line {
    fn label_value(label: &str, value: &str) -> Self {
        Self::LabelValue(label.to_string(), value.to_string())
    }
}

struct Colors {
    banner: Color,
    accent: Color,
    border: Color,
    text: Color,
    muted: Color,
    background: Color,
    selection_bg: Color,
    selection_fg: Color,
}

impl Colors {
    fn active() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            Self {
                banner: Color::White,
                accent: Color::White,
                border: Color::White,
                text: Color::White,
                muted: Color::White,
                background: Color::Reset,
                selection_bg: Color::Grey,
                selection_fg: Color::Black,
            }
        } else {
            Self {
                banner: Color::Magenta,
                accent: Color::Yellow,
                border: Color::DarkGrey,
                text: Color::White,
                muted: Color::DarkGrey,
                background: Color::Reset,
                selection_bg: Color::DarkYellow,
                selection_fg: Color::Black,
            }
        }
    }
}
