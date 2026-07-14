//! Ratatui frame draw via `bevy_ratatui::RatatuiContext`.

use {
    super::{
        layout::split_frame,
        scrollback::ScrollbackView,
        state::{TuiFocus, TuiState},
    },
    bevy::ecs::change_detection::{Res, ResMut},
    bevy_ratatui::RatatuiContext,
    ratatui::{
        style::{Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph, Wrap},
    },
};

pub fn draw_system(
    mut context: ResMut<RatatuiContext>,
    state: Res<TuiState>,
    scrollback: Res<ScrollbackView>,
) -> bevy::prelude::Result {
    let focus = state.focus;
    let scroll_from_bottom = state.scroll_from_bottom;
    let prompt = state.prompt.clone();
    let status_text = state.status_lines.last().cloned().unwrap_or_else(|| {
        "greatsage tui · Tab focus · Enter submit · Ctrl+C / q quit".to_string()
    });
    let empty = scrollback.empty_placeholder || scrollback.lines.is_empty();
    let body_lines: Vec<String> = if empty {
        vec!["(empty session — type a prompt below)".to_string()]
    } else {
        scrollback.lines.iter().map(|l| l.text.clone()).collect()
    };

    context.draw(|frame| {
        let areas = split_frame(frame.area());

        let scroll_title = match focus {
            TuiFocus::Scrollback => " scrollback (focused) ",
            TuiFocus::Prompt => " scrollback ",
        };
        let body: Vec<Line> = if empty {
            vec![Line::from(Span::styled(
                body_lines[0].as_str(),
                Style::default().add_modifier(Modifier::DIM),
            ))]
        } else {
            body_lines.iter().map(|t| Line::from(t.as_str())).collect()
        };
        let scroll_widget = Paragraph::new(body)
            .block(Block::default().borders(Borders::ALL).title(scroll_title))
            .wrap(Wrap { trim: false })
            .scroll((scroll_from_bottom, 0));
        () = frame.render_widget(scroll_widget, areas.scrollback);

        () = frame.render_widget(
            Paragraph::new(status_text.as_str())
                .style(Style::default().add_modifier(Modifier::DIM)),
            areas.status,
        );

        let prompt_title = match focus {
            TuiFocus::Prompt => " prompt (focused) ",
            TuiFocus::Scrollback => " prompt ",
        };
        let prompt_display = format!("> {prompt}");
        () = frame.render_widget(
            Paragraph::new(prompt_display)
                .block(Block::default().borders(Borders::ALL).title(prompt_title)),
            areas.prompt,
        );
    })?;

    Ok(())
}
