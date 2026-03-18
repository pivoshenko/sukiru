use std::io::IsTerminal;
use unicode_width::UnicodeWidthStr;

const BANNER_TOP: &str = "╔═══════════════════════════════════════════════════════════════╗";
const BANNER_BOTTOM: &str = "╚═══════════════════════════════════════════════════════════════╝";
const BANNER_INNER_WIDTH: usize = 63;
const LOGO_LINES: [&str; 6] = [
    "  ██╗  ██╗ █████╗ ███████╗███████╗████████╗████████╗ ██████╗   ",
    "  ██║ ██╔╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝╚══██╔══╝██╔═══██╗  ",
    "  █████╔╝ ███████║███████╗█████╗     ██║      ██║   ██║   ██║  ",
    "  ██╔═██╗ ██╔══██║╚════██║██╔══╝     ██║      ██║   ██║   ██║  ",
    "  ██║  ██╗██║  ██║███████║███████╗   ██║      ██║   ╚██████╔╝  ",
    "  ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝      ╚═╝    ╚═════╝   ",
];
const JAPANESE_SUBTITLE: &str = "スキル・パッケージ・マネージャー";

fn color_stdout_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn empty_banner_line() -> String {
    format!("║{}║", " ".repeat(BANNER_INNER_WIDTH))
}

fn left_boxed_line(content: &str) -> String {
    let width = UnicodeWidthStr::width(content);
    let right_pad = BANNER_INNER_WIDTH.saturating_sub(width);
    format!("║{}{}║", content, " ".repeat(right_pad))
}

fn centered_boxed_line(content: &str) -> String {
    let width = UnicodeWidthStr::width(content);
    let total_pad = BANNER_INNER_WIDTH.saturating_sub(width);
    let left_pad = total_pad / 2;
    let right_pad = total_pad - left_pad;
    format!(
        "║{}{}{}║",
        " ".repeat(left_pad),
        content,
        " ".repeat(right_pad)
    )
}

fn colorize_content(line: &str, content: &str, color: &str, base: &str) -> String {
    line.replacen(content, &format!("{color}{content}{base}"), 1)
}

pub fn banner_lines() -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(BANNER_TOP.to_string());
    for logo in LOGO_LINES {
        lines.push(left_boxed_line(logo));
    }
    lines.push(empty_banner_line());
    lines.push(centered_boxed_line(JAPANESE_SUBTITLE));
    lines.push(BANNER_BOTTOM.to_string());
    lines
}

pub fn banner_width() -> usize {
    UnicodeWidthStr::width(BANNER_TOP)
}

pub fn banner_string(use_color: bool) -> String {
    let mut lines = banner_lines();
    if use_color {
        if let Some(subtitle) = lines.get_mut(LOGO_LINES.len() + 2) {
            *subtitle = colorize_content(subtitle, JAPANESE_SUBTITLE, "\x1b[90m", "\x1b[95m");
        }
        format!("\x1b[95m{}\x1b[0m\n", lines.join("\n"))
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

pub fn print_banner() {
    print!("{}", banner_string(color_stdout_enabled()));
}
