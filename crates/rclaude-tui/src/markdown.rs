//! Basic Markdown rendering for terminal output.
//! Converts markdown text to styled ratatui Spans.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Convert markdown text to styled Lines for ratatui.
pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    let mut _code_lang = String::new();

    for line in text.lines() {
        if line.starts_with("```") {
            if in_code_block {
                in_code_block = false;
                lines.push(Line::styled("```", Style::default().fg(Color::DarkGray)));
            } else {
                in_code_block = true;
                _code_lang = line.strip_prefix("```").unwrap_or("").trim().to_string();
                lines.push(Line::styled(
                    format!("```{_code_lang}"),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            continue;
        }

        if in_code_block {
            lines.push(Line::styled(
                line.to_string(),
                Style::default().fg(Color::Green),
            ));
            continue;
        }

        // Headers
        if line.starts_with("### ") {
            lines.push(Line::styled(
                line.strip_prefix("### ").unwrap_or(line).to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if line.starts_with("## ") {
            lines.push(Line::styled(
                line.strip_prefix("## ").unwrap_or(line).to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        } else if line.starts_with("# ") {
            lines.push(Line::styled(
                line.strip_prefix("# ").unwrap_or(line).to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        }
        // Bullet points
        else if line.starts_with("- ") || line.starts_with("* ") {
            let mut spans = vec![Span::styled("• ", Style::default().fg(Color::Cyan))];
            spans.extend(render_inline_markdown(
                line.strip_prefix("- ")
                    .or(line.strip_prefix("* "))
                    .unwrap_or(line),
            ));
            lines.push(Line::from(spans));
        }
        // Numbered lists
        else if line.len() > 2
            && line.chars().next().is_some_and(|c| c.is_ascii_digit())
            && line.contains(". ")
        {
            let mut spans = vec![Span::styled(
                line.split(". ").next().unwrap_or("").to_string() + ". ",
                Style::default().fg(Color::Cyan),
            )];
            if let Some(rest) = line.split_once(". ").map(|(_, r)| r) {
                spans.extend(render_inline_markdown(rest));
            }
            lines.push(Line::from(spans));
        }
        // Regular text with inline formatting
        else {
            let spans = render_inline_markdown(line);
            lines.push(Line::from(spans));
        }
    }
    lines
}

/// Render inline markdown (bold, italic, code) to Spans.
fn render_inline_markdown(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Inline code: `code`
        if chars[i] == '`' {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            i += 1;
            let mut code = String::new();
            while i < chars.len() && chars[i] != '`' {
                code.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            } // skip closing `
            spans.push(Span::styled(code, Style::default().fg(Color::Green)));
            continue;
        }

        // Bold: **text**
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            i += 2;
            let mut bold = String::new();
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '*') {
                bold.push(chars[i]);
                i += 1;
            }
            if i + 1 < chars.len() {
                i += 2;
            } // skip closing **
            spans.push(Span::styled(
                bold,
                Style::default().add_modifier(Modifier::BOLD),
            ));
            continue;
        }

        current.push(chars[i]);
        i += 1;
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }
    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_header() {
        let lines = render_markdown("# Hello");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_render_code_block() {
        let lines = render_markdown("```rust\nfn main() {}\n```");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_render_bullet() {
        let lines = render_markdown("- item one\n- item two");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_inline_code() {
        let spans = render_inline_markdown("use `cargo build` to compile");
        assert!(spans.len() >= 3); // text + code + text
    }

    #[test]
    fn test_inline_bold() {
        let spans = render_inline_markdown("this is **bold** text");
        assert!(spans.len() >= 3);
    }
}
