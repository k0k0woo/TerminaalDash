use crate::{app::{App, InputMode}, shared::standard_block};
use ratatui::{
    buffer::Buffer, layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ScheduleItem { pub time: String, pub activity: String }

#[derive(Serialize, Deserialize, Clone)]
pub struct ReminderItem { pub task: String, pub is_done: bool }

pub struct ScheduleWidget<'a> { pub schedule: &'a [ScheduleItem], pub is_selected: bool }
impl<'a> Widget for ScheduleWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self.schedule.iter().map(|i| ListItem::new(format!(" {} | {}", i.time, i.activity))).collect();
        List::new(items).block(standard_block(" Schedule ", self.is_selected)).render(area, buf);
    }
}

pub struct RemindersWidget<'a> { pub reminders: &'a [ReminderItem], pub active_idx: usize, pub is_selected: bool }
impl<'a> Widget for RemindersWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self.reminders.iter().enumerate().map(|(i, r)| {
            let sym = if r.is_done { "[x]" } else { "[ ]" };
            let style = if i == self.active_idx && self.is_selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() };
            ListItem::new(format!(" {} {}", sym, r.task)).style(style)
        }).collect();
        List::new(items).block(standard_block(" Reminders (Space) ", self.is_selected)).render(area, buf);
    }
}


pub struct FocusedRemindersWidget<'a> { pub app: &'a App }
impl<'a> Widget for FocusedRemindersWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = standard_block(" Interactive Reminders Manager ", true);
        let inner = block.inner(area);
        block.render(area, buf);
        let layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(0), Constraint::Length(3)]).split(inner);
        let items: Vec<ListItem> = self.app.reminders.iter().enumerate().map(|(i, r)| { let sym = if r.is_done { "[x]" } else { "[ ]" }; let style = if i == self.app.reminder_index { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() }; ListItem::new(format!(" {} {}", sym, r.task)).style(style) }).collect();
        List::new(items).render(layout[0], buf);
        let bottom_text = match self.app.input_mode { InputMode::AddingReminder => format!("New Reminder: {}█", self.app.input_buffer), _ => "Keys: [a] Add  |  [d]/[Backspace] Delete  |  [Space]/[Enter] Toggle  |  [Esc] Exit".to_string(), };
        Paragraph::new(bottom_text).block(Block::default().borders(Borders::TOP)).render(layout[1], buf);
    }
}

pub struct FocusedScheduleWidget<'a> { pub app: &'a App }
impl<'a> Widget for FocusedScheduleWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = standard_block(" Interactive Calendar Manager ", true);
        let inner = block.inner(area);
        block.render(area, buf);
        let layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(0), Constraint::Length(3)]).split(inner);
        let items: Vec<ListItem> = self.app.schedule.iter().enumerate().map(|(i, s)| { let style = if i == self.app.schedule_index { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() }; ListItem::new(format!(" [{}] {}", s.time, s.activity)).style(style) }).collect();
        List::new(items).render(layout[0], buf);
        let bottom_text = match self.app.input_mode { InputMode::AddingScheduleTime => format!("Enter Time (e.g., 14:30): {}█", self.app.input_buffer), InputMode::AddingScheduleActivity => format!("Time: {} | Enter Activity: {}█", self.app.temp_schedule_time, self.app.input_buffer), _ => "Keys: [a] Add  |  [d]/[Backspace] Delete  |  [Esc] Exit".to_string(), };
        Paragraph::new(bottom_text).block(Block::default().borders(Borders::TOP)).render(layout[1], buf);
    }
}