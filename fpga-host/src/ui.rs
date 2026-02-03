//! UI rendering logic
//!
//! This module contains all the ratatui widget rendering code.

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Render the full UI
pub fn render(frame: &mut Frame, app: &App) {
    // Create main layout: log area + input line
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Log area (takes remaining space)
            Constraint::Length(3), // Input area (fixed height)
        ])
        .split(frame.area());

    render_log_area(frame, app, chunks[0]);
    render_input_area(frame, app, chunks[1]);
}

/// Render the scrolling log area
fn render_log_area(frame: &mut Frame, app: &App, area: Rect) {
    // Calculate how many lines can fit in the log area
    // Subtract 2 for the border
    let visible_lines = area.height.saturating_sub(2) as usize;

    // Convert log messages to list items with appropriate styling
    let total_messages = app.log_messages.len();
    let start_idx = if app.scroll_offset == 0 {
        // Auto-scroll: show the last N messages
        total_messages.saturating_sub(visible_lines)
    } else {
        // Manual scroll: show from calculated offset
        total_messages
            .saturating_sub(visible_lines)
            .saturating_sub(app.scroll_offset)
    };

    let items: Vec<ListItem> = app
        .log_messages
        .iter()
        .skip(start_idx)
        .take(visible_lines)
        .map(|log_line| {
            let style = match log_line.level {
                log::Level::Error => Style::default().fg(Color::Red),
                log::Level::Warn => Style::default().fg(Color::Yellow),
                log::Level::Info => Style::default().fg(Color::White),
                log::Level::Debug => Style::default().fg(Color::Gray),
                log::Level::Trace => Style::default().fg(Color::DarkGray),
            };

            let level_str = format!("[{:5}]", log_line.level);
            let line = Line::from(vec![
                Span::styled(level_str, style.add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(&log_line.message, style),
            ]);
            ListItem::new(line)
        })
        .collect();

    // Build title with scroll indicator
    let title = if app.scroll_offset > 0 {
        format!(
            " FPGA Host Interface (scroll: -{}, ESC to reset) ",
            app.scroll_offset
        )
    } else {
        " FPGA Host Interface ".to_string()
    };

    let log_list = List::new(items).block(Block::default().title(title).borders(Borders::ALL));

    frame.render_widget(log_list, area);
}

/// Render the command input area
fn render_input_area(frame: &mut Frame, app: &App, area: Rect) {
    // Build prompt based on connection status
    let prompt = if app.is_connected() {
        Span::styled("[CONNECTED] > ", Style::default().fg(Color::Green))
    } else {
        Span::styled("[DISCONNECTED] > ", Style::default().fg(Color::Red))
    };

    let input_line = Line::from(vec![
        prompt,
        Span::raw(&app.input_buffer),
        Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
    ]);

    let input_paragraph = Paragraph::new(input_line).block(Block::default().borders(Borders::ALL));

    frame.render_widget(input_paragraph, area);
}
