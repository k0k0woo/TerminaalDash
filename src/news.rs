use crate::{app::AppEvent, shared::standard_block};
use ratatui::{buffer::Buffer, layout::Rect, widgets::{Paragraph, Widget, Wrap}};
use std::{sync::mpsc, time::Duration};

pub async fn fetch_news_data(tx: mpsc::Sender<AppEvent>) {
    let feeds = vec!["https://www.investing.com/rss/news.rss", "https://news.ycombinator.com/rss"];
    let client = reqwest::Client::builder().user_agent("Mozilla/5.0").build().unwrap_or_default();
    loop {
        let mut headlines = Vec::new();
        for url in &feeds {
            if let Ok(res) = client.get(*url).send().await {
                if let Ok(bytes) = res.bytes().await {
                    if let Ok(channel) = rss::Channel::read_from(&bytes[..]) {
                        let channel_name = channel.title().split('-').next().unwrap_or("News").trim();
                        for item in channel.items().iter().take(7) {
                            if let Some(title) = item.title() { headlines.push(format!("• {}: {}", channel_name, title)); }
                        }
                    }
                }
            }
        }
        let _ = tx.send(AppEvent::UpdateNews(format!("\n{}", headlines.join("\n"))));
        tokio::time::sleep(Duration::from_secs(1200)).await;
    }
}

pub struct NewsWidget<'a> { pub text: &'a str, pub is_selected: bool }
impl<'a> Widget for NewsWidget<'a> { 
    fn render(self, area: Rect, buf: &mut Buffer) { Paragraph::new(self.text).block(standard_block(" Top News ", self.is_selected)).wrap(Wrap { trim: true }).render(area, buf); } 
}