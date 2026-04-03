//! TUI rendering with ratatui.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{ChatRole, TuiState};

/// Render the full TUI layout.
pub fn render(frame: &mut Frame, state: &TuiState) {
    let area = frame.area();

    // Layout: [chat area] [status bar] [input area]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // chat
            Constraint::Length(1), // status bar
            Constraint::Length(3), // input
        ])
        .split(area);

    render_chat(frame, state, chunks[0]);
    render_status_bar(frame, state, chunks[1]);
    render_input(frame, state, chunks[2]);
}

fn render_chat(frame: &mut Frame, state: &TuiState, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.messages {
        let (prefix, style) = match msg.role {
            ChatRole::User => (
                "> ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            ChatRole::Assistant => ("", Style::default().fg(Color::White)),
            ChatRole::System => (
                "[system] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::DIM),
            ),
            ChatRole::Tool => (
                "[tool] ",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
            ),
        };

        // Add role prefix on first line
        let content_lines: Vec<&str> = msg.content.lines().collect();
        if let Some(first) = content_lines.first() {
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(*first, style),
            ]));
        }
        for line in content_lines.iter().skip(1) {
            lines.push(Line::styled(*line, style));
        }
        lines.push(Line::raw("")); // blank line between messages
    }

    // Loading indicator with Q11 status message
    if state.is_loading {
        let status = state.loading_status.as_deref().unwrap_or("Thinking...");
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", state.spinner()),
                Style::default().fg(Color::Magenta),
            ),
            Span::styled(status, Style::default().fg(Color::DarkGray)),
        ]));
    }

    let chat = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .scroll((state.scroll_offset, 0));

    frame.render_widget(chat, area);
}

fn render_status_bar(frame: &mut Frame, state: &TuiState, area: Rect) {
    let model_short = state
        .status_model
        .split('-')
        .take(2)
        .collect::<Vec<_>>()
        .join("-");

    let mut parts = vec![Span::styled(
        format!(" {model_short} "),
        Style::default().fg(Color::Black).bg(Color::Cyan),
    )];

    if let Some(ref branch) = state.status_branch {
        parts.push(Span::styled(
            format!("  {branch} "),
            Style::default().fg(Color::Black).bg(Color::Green),
        ));
    }

    parts.push(Span::styled(
        format!(" ${:.4} ", state.status_cost),
        Style::default().fg(Color::DarkGray),
    ));

    if state.status_tokens > 0 {
        parts.push(Span::styled(
            format!(" {}tok ", state.status_tokens),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let bar = Paragraph::new(Line::from(parts)).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(bar, area);
}

fn render_input(frame: &mut Frame, state: &TuiState, area: Rect) {
    let input = Paragraph::new(state.input.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if state.is_loading {
                Color::DarkGray
            } else {
                Color::Green
            }))
            .title(if state.is_loading {
                " waiting... "
            } else {
                " > "
            }),
    );

    frame.render_widget(input, area);

    // Place cursor
    if !state.is_loading {
        let cursor_x = area.x + 1 + state.cursor_pos as u16;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x.min(area.right() - 2), cursor_y));
    }
}
