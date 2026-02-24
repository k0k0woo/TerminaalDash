use crate::{app::AppEvent, shared::standard_block};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::{Paragraph, Widget},
};
use std::{sync::mpsc, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeatherCondition { Clear, Clouds, Rain, Storm, Snow, Unknown }

pub async fn fetch_weather_data(tx: mpsc::Sender<AppEvent>) {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("OPENWEATHER_API_KEY").unwrap_or_else(|_| "demo".to_string());
    let city = std::env::var("CITY").unwrap_or_else(|_| "Lincoln, UK".to_string());
    loop {
        let url = format!("https://api.openweathermap.org/data/2.5/weather?q={}&appid={}&units=metric", city, api_key);
        if let Ok(res) = reqwest::get(&url).await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(weather_array) = json.get("weather").and_then(|w| w.as_array()) {
                    if let Some(weather_obj) = weather_array.get(0) {
                        let desc = weather_obj.get("main").and_then(|v| v.as_str()).unwrap_or("Unknown");
                        let temp = json.get("main").and_then(|m| m.get("temp")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                        
                        let desc_lower = desc.to_lowercase();
                        let condition = if desc_lower.contains("storm") || desc_lower.contains("thunderstorm") { WeatherCondition::Storm } 
                        else if desc_lower.contains("snow") { WeatherCondition::Snow } 
                        else if desc_lower.contains("rain") || desc_lower.contains("drizzle") { WeatherCondition::Rain } 
                        else if desc_lower.contains("cloud") { WeatherCondition::Clouds } 
                        else { WeatherCondition::Clear };

                        let emoji = match condition {
                            WeatherCondition::Storm => "🌩️", WeatherCondition::Snow => "❄️",
                            WeatherCondition::Rain => "🌧️", WeatherCondition::Clouds => "☁️",
                            WeatherCondition::Clear => "☀️", WeatherCondition::Unknown => "🌡️",
                        };

                        let formatted = format!("{}\n{} {:.1}°C\n{}", json.get("name").and_then(|v| v.as_str()).unwrap_or(&city), emoji, temp, desc);
                        let _ = tx.send(AppEvent::UpdateWeather(formatted, condition));
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

pub struct WeatherWidget<'a> { 
    pub text: &'a str, 
    pub is_selected: bool,
    pub tick_count: u64,
    pub condition: WeatherCondition,
}

impl<'a> Widget for WeatherWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = standard_block(" Weather ", self.is_selected);
        let inner_area = block.inner(area);
        block.render(area, buf);

        Paragraph::new(self.text)
            .alignment(ratatui::layout::Alignment::Center)
            .render(inner_area, buf);

        if inner_area.height == 0 || inner_area.width == 0 { return; }

        match self.condition {
            WeatherCondition::Rain | WeatherCondition::Storm => {
                let is_storm = self.condition == WeatherCondition::Storm;
                let speed = if is_storm { 2 } else { 1 };
                let symbol = if is_storm { "/" } else { "|" };
                let color = if is_storm { Color::Blue } else { Color::Cyan };
                
                for x in inner_area.left()..inner_area.right() {
                    let col_seed = (x as u64).wrapping_mul(1103515245);
                    if col_seed % 10 > 4 { continue; }
                    let drop_y = inner_area.top() + (((self.tick_count * speed) + col_seed % 100) % inner_area.height as u64) as u16;
                    if let Some(cell) = buf.cell_mut((x, drop_y)) {
                        if cell.symbol() == " " { cell.set_symbol(symbol).set_fg(color); }
                    }
                }
                
                if is_storm && self.tick_count % 80 < 2 {
                    for y in inner_area.top()..inner_area.bottom() {
                        for x in inner_area.left()..inner_area.right() {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                if cell.symbol() == " " && (x + y) % 3 == 0 { cell.set_bg(Color::DarkGray); }
                            }
                        }
                    }
                }
            }
            WeatherCondition::Snow => {
                for x in inner_area.left()..inner_area.right() {
                    let col_seed = (x as u64).wrapping_mul(123456789);
                    if col_seed % 10 > 2 { continue; } 
                    let drop_y = inner_area.top() + (((self.tick_count / 2) + col_seed % 100) % inner_area.height as u64) as u16;
                    let drift = (self.tick_count / 4 + col_seed) % 3; 
                    let drop_x = (x + drift as u16).clamp(inner_area.left(), inner_area.right() - 1);
                    if let Some(cell) = buf.cell_mut((drop_x, drop_y)) {
                        if cell.symbol() == " " { cell.set_symbol("*").set_fg(Color::White); }
                    }
                }
            }
            WeatherCondition::Clouds => {
                for y in inner_area.top()..inner_area.bottom() {
                    let row_seed = (y as u64).wrapping_mul(987654321);
                    if row_seed % 4 != 0 { continue; } 
                    let x_pos = inner_area.left() + (((self.tick_count / 2) + row_seed % 100) % inner_area.width as u64) as u16;
                    if let Some(cell) = buf.cell_mut((x_pos, y)) {
                        if cell.symbol() == " " { cell.set_symbol("~~~").set_fg(Color::DarkGray); }
                    }
                }
            }
            WeatherCondition::Clear => {
                for y in inner_area.top()..inner_area.bottom() {
                    for x in inner_area.left()..inner_area.right() {
                        let seed = (x as u64).wrapping_mul(111).wrapping_add(y as u64 * 222);
                        if seed % 40 == 0 && (self.tick_count + seed) % 40 < 20 {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                if cell.symbol() == " " { cell.set_symbol("+").set_fg(Color::Yellow); }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}