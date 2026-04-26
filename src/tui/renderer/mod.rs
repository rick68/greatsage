pub mod display_utils;
pub mod widgets;

use {
    crate::{
        agents::CodingAgentTotalTokenUsage,
        tui::{
            core::{
                COLOR_BORDER_FOCUSED, COLOR_BORDER_UNFOCUSED, PROMPT_PREFIX, SPINNER, TuiMain,
                TuiMainFocus,
            },
            events::RenderNeeded,
        },
    },
    bevy::{
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            system::Local,
        },
        time::{Time, Timer, TimerMode},
    },
    bevy_ratatui::RatatuiContext,
    ratatui::{
        layout::{Constraint, Layout},
        style::Style,
        widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation},
        Frame,
    },
    std::time::Duration,
};

pub fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    mut tui: NonSendMut<TuiMain>,
    time: Res<Time<()>>,
    mut cursor_timer: Local<Option<Timer>>,
    mut spinner_timer: Local<Option<Timer>>,
    mut dirty: ResMut<RenderNeeded>,
    token_usage: Option<Res<CodingAgentTotalTokenUsage>>,
) -> bevy::ecs::error::Result {
    let cursor_timer = cursor_timer.get_or_insert(Timer::new(
        Duration::from_millis(crate::tui::core::CURSOR_BLINK_INTERVAL_MS),
        TimerMode::Repeating,
    ));
    _ = cursor_timer.tick(time.delta());
    if cursor_timer.just_finished() {
        tui.show_cursor ^= true;
        **dirty = true;
    }

    let spinner_timer =
        spinner_timer.get_or_insert(Timer::new(Duration::from_millis(100), TimerMode::Repeating));
    _ = spinner_timer.tick(time.delta());
    let has_spinner = tui.blocks.iter().any(|b| match b {
        crate::tui::core::OutputBlock::Response(resp) => resp.has_spinner(),
        _ => false,
    });
    if has_spinner && spinner_timer.just_finished() {
        **dirty = true;
    }

    if **dirty {
        _ = context.draw(|frame| {
            render_tui(frame, &mut tui, token_usage.as_deref());
        })?;
    }
    **dirty = false;
    Ok(())
}

fn render_tui(
    frame: &mut Frame,
    tui: &mut TuiMain,
    token_usage: Option<&CodingAgentTotalTokenUsage>,
) {
    let area = frame.area();
    let inner_width = area.width.saturating_sub(2) as usize;
    let input_lines = display_utils::input_display_lines(tui, inner_width, PROMPT_PREFIX);
    let input_height = (input_lines.len() as u16 + 2).max(3);
    let vertical = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(input_height),
    ]);
    let [output_area, status_area, input_area] = vertical.areas(area);
    tui.output_area = output_area;
    let output_inner_width = output_area.width.saturating_sub(2) as usize;
    let (flat, flat_map) = display_utils::rendered_flat_lines(tui, &SPINNER);
    let (wrapped, wrapped_map) =
        display_utils::hard_wrap_output_lines_with_map(&flat, &flat_map, output_inner_width);
    tui.line_map = wrapped_map;
    let total_rows = wrapped.len();
    let output_height = output_area.height.saturating_sub(2) as usize;
    let new_max = total_rows.saturating_sub(output_height);
    tui.vertical_scroll = tui.vertical_scroll.min(new_max);
    let output_border_color = if tui.focused == TuiMainFocus::OutputArea { COLOR_BORDER_FOCUSED } else { COLOR_BORDER_UNFOCUSED };
    let output = Paragraph::new(wrapped).style(Style::default()).block(Block::bordered().title("Output").border_style(Style::default().fg(output_border_color))).scroll((tui.vertical_scroll as u16, 0));
    let scroll_positions = total_rows.saturating_sub(output_height) + 1;
    tui.vertical_scroll_state = tui.vertical_scroll_state.content_length(scroll_positions).viewport_content_length(output_height).position(tui.vertical_scroll);
    frame.render_widget(output, output_area);
    frame.render_stateful_widget(Scrollbar::new(ScrollbarOrientation::VerticalRight).begin_symbol(Some("↑")).end_symbol(Some("↓")), output_area, &mut tui.vertical_scroll_state);
    let status_text = if let Some(usage) = token_usage {
        let CodingAgentTotalTokenUsage(usage) = usage;
        format!(" 🎯 Input: {} | Output: {} | Cache Read: {} | Cache Write: {}", usage.input, usage.output, usage.cache_read, usage.cache_write)
    } else { " 🎯 Token usage: Waiting for first response...".to_string() };
    let status = Paragraph::new(status_text).style(Style::default().fg(ratatui::style::Color::Rgb(100, 150, 200))).block(Block::bordered().title("Token Usage"));
    frame.render_widget(status, status_area);
    let input_text: Vec<ratatui::text::Line<'_>> = input_lines.iter().map(|l| ratatui::text::Line::from(l.as_str())).collect();
    let input_border_color = if tui.focused == TuiMainFocus::InputArea { COLOR_BORDER_FOCUSED } else { COLOR_BORDER_UNFOCUSED };
    let input = Paragraph::new(input_text).style(Style::default()).block(Block::bordered().title("Input").border_style(Style::default().fg(input_border_color)));
    frame.render_widget(input, input_area);
    if tui.show_cursor && tui.focused == TuiMainFocus::InputArea {
        use unicode_width::UnicodeWidthStr;
        let cursor_total = PROMPT_PREFIX.width() + display_utils::display_index(tui);
        let (cursor_row, cursor_col) = {
            let mut accumulated = 0usize;
            let mut result = (0usize, cursor_total);
            for (row, line) in input_lines.iter().enumerate() {
                let line_w = line.width();
                if cursor_total <= accumulated + line_w { result = (row, cursor_total - accumulated); break; }
                if row + 1 < input_lines.len() { accumulated += line_w; }
                else { result = (row, cursor_total - accumulated); }
            }
            result
        };
        frame.set_cursor_position((input_area.left() + cursor_col as u16 + 1, input_area.top() + cursor_row as u16 + 1));
    }
}
