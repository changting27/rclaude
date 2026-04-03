//! Interactive permission prompt with arrow-key selection.
//! Matches claude's ● ○ radio-button style permission UI.

use std::io::Write;

/// Permission choice result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionChoice {
    AllowOnce,
    AllowAlways,
    Deny,
    DenyAlways,
}

/// Options for the permission prompt.
const OPTIONS: &[(PermissionChoice, &str)] = &[
    (PermissionChoice::AllowOnce, "Yes, allow once"),
    (PermissionChoice::AllowAlways, "Yes, always allow"),
    (PermissionChoice::Deny, "No, deny"),
    (PermissionChoice::DenyAlways, "No, always deny"),
];

/// Show an interactive permission prompt with arrow-key selection.
pub fn show_permission_select(tool_name: &str, description: &str) -> PermissionChoice {
    if !atty::is(atty::Stream::Stdin) {
        eprintln!("  Permission denied (non-interactive): {description}");
        return PermissionChoice::Deny;
    }

    match show_interactive(tool_name, description) {
        Some(choice) => choice,
        None => show_text_fallback(tool_name, description),
    }
}

fn show_interactive(tool_name: &str, description: &str) -> Option<PermissionChoice> {
    use crossterm::{event, terminal};

    terminal::enable_raw_mode().ok()?;

    let mut selected: usize = 0;
    let stderr = std::io::stderr();
    let mut out = stderr.lock();

    write!(out, "\r\n  Allow {tool_name}: {description}\r\n\r\n").ok()?;
    draw_options(&mut out, selected);

    let result = loop {
        // Blocking read — instant response, no polling delay
        let ev = match event::read() {
            Ok(ev) => ev,
            Err(_) => break None,
        };

        if let event::Event::Key(key) = ev {
            match key.code {
                event::KeyCode::Up | event::KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                    draw_options(&mut out, selected);
                }
                event::KeyCode::Down | event::KeyCode::Char('j') => {
                    if selected < OPTIONS.len() - 1 {
                        selected += 1;
                    }
                    draw_options(&mut out, selected);
                }
                event::KeyCode::Enter | event::KeyCode::Char(' ') => {
                    break Some(OPTIONS[selected].0);
                }
                event::KeyCode::Char('y') => break Some(PermissionChoice::AllowOnce),
                event::KeyCode::Char('a') => break Some(PermissionChoice::AllowAlways),
                event::KeyCode::Char('n') | event::KeyCode::Esc => {
                    break Some(PermissionChoice::Deny)
                }
                event::KeyCode::Char('d') => break Some(PermissionChoice::DenyAlways),
                event::KeyCode::Char('c')
                    if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                {
                    break Some(PermissionChoice::Deny)
                }
                _ => {}
            }
        }
    };

    // Clear options area and restore terminal
    for _ in 0..OPTIONS.len() {
        write!(out, "\r\n").ok();
    }
    out.flush().ok();
    terminal::disable_raw_mode().ok();

    result
}

fn draw_options(out: &mut std::io::StderrLock, selected: usize) {
    // Always redraw from top
    write!(out, "{}", crossterm::cursor::MoveUp(OPTIONS.len() as u16)).ok();
    for (i, (_, label)) in OPTIONS.iter().enumerate() {
        let marker = if i == selected { "●" } else { "○" };
        // Clear line and write option
        write!(
            out,
            "\r{}  {marker} {label}  \r\n",
            crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)
        )
        .ok();
    }
    // Move back to top of options
    write!(out, "{}", crossterm::cursor::MoveUp(OPTIONS.len() as u16)).ok();
    // Move to selected line
    if selected > 0 {
        write!(out, "{}", crossterm::cursor::MoveDown(selected as u16)).ok();
    }
    out.flush().ok();
}

fn show_text_fallback(tool_name: &str, description: &str) -> PermissionChoice {
    eprintln!("\n  Allow {tool_name}: {description}");
    eprint!("  [y]es / [n]o / [a]lways / [d]eny always: ");
    std::io::stderr().flush().ok();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return PermissionChoice::Deny;
    }
    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => PermissionChoice::AllowOnce,
        "a" | "always" => PermissionChoice::AllowAlways,
        "d" | "deny" => PermissionChoice::DenyAlways,
        _ => PermissionChoice::Deny,
    }
}
