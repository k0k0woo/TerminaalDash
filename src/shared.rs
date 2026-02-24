use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};
use crate::app::AppEvent;
use chrono::Local;
use rand::{Rng, RngExt};
use std::{sync::mpsc, time::Duration};

pub fn load_json<T: Default + serde::de::DeserializeOwned>(path: &str) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn standard_block<'a>(title: &'a str, is_selected: bool) -> Block<'a> {
    let style = if is_selected { Style::default().fg(Color::White) } else { Style::default().fg(Color::DarkGray) };
    let b_type = if is_selected { BorderType::Thick } else { BorderType::Rounded };
    Block::default().title(title).borders(Borders::ALL).border_type(b_type).border_style(style)
}

pub async fn fetch_time(tx: mpsc::Sender<AppEvent>) {
    loop {
        let now = Local::now();
        let formatted = now.format("%d %b %Y • %H:%M:%S ").to_string();
        let _ = tx.send(AppEvent::UpdateTime(formatted));
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub struct TimeWidget<'a> { pub text: &'a str }
impl<'a> Widget for TimeWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) { 
        Paragraph::new(self.text).block(standard_block(" Time ", false)).alignment(ratatui::layout::Alignment::Center).render(area, buf); 
    }
}

pub struct FocusedViewWidget<'a> { pub title: &'a str }
impl<'a> Widget for FocusedViewWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) { 
        Paragraph::new("\n\nDetailed view goes here.\n\nPress 'Esc' or Click anywhere to return.").block(standard_block(self.title, true)).alignment(ratatui::layout::Alignment::Center).render(area, buf); 
    }
}
