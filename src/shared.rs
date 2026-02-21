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

pub struct MatrixEdgeOverlay { pub tick: u64 }
impl Widget for MatrixEdgeOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut local_rng = rand::rng();
        for x in area.left()..area.right() {
            let col_seed = (x as u64).wrapping_mul(1103515245);
            if col_seed % 100 > 75 { continue; }
            let head_y = ((self.tick / ((col_seed % 3) + 1)) as i64 + (col_seed % 100) as i64) % (area.height as i64 + 20) - 10;
            for y in area.top()..area.bottom() {
                let dist_x = std::cmp::min(x - area.left(), area.right() - 1 - x);
                let dist_y = std::cmp::min(y - area.top(), area.bottom() - 1 - y);
                if dist_x > 3 && dist_y > 1 { continue; }
                let dist_to_head = head_y - y as i64;
                if dist_to_head >= 0 && dist_to_head < 15 {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        let chars = ['0','1','2','3','4','5','6','7','8','9','A','B','C','Z','X','Y','W','*','+','=','-',':','.','"','$','%','&'];
                        let c = chars[local_rng.random_range(0..chars.len())];
                        cell.set_symbol(&c.to_string());
                        if dist_to_head == 0 { cell.set_fg(Color::White); } else if dist_to_head < 3 { cell.set_fg(Color::LightGreen); } else if dist_to_head > 11 { cell.set_fg(Color::DarkGray); } else { cell.set_fg(Color::Green); }
                    }
                }
            }
        }
    }
}