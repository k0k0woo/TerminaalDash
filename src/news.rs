use crate::{app::AppEvent, shared::standard_block};
use ratatui::{
    buffer::Buffer, 
    layout::Rect, 
    widgets::{Paragraph, Widget, Wrap}
};
use std::{sync::mpsc, time::Duration};

pub async fn fetch_news_data(tx: mpsc::Sender<AppEvent>) {
    let feeds = vec![
        "https://www.investing.com/rss/news.rss", 
        "https://news.ycombinator.com/rss",
        "https://feeds.bbci.co.uk/news/rss.xml",
        "https://feeds.content.dowjones.io/public/rss/mw_realtimeheadlines",
        "https://feeds.content.dowjones.io/public/rss/mw_bulletins",
        "https://www.investing.com/rss/news_25.rss"

    ];
    let client = reqwest::Client::builder().user_agent("Mozilla/5.0").build().unwrap_or_default();
    
    loop {
        let mut headlines = Vec::new();
        for url in &feeds {
            if let Ok(res) = client.get(*url).send().await {
                if let Ok(bytes) = res.bytes().await {
                    if let Ok(channel) = rss::Channel::read_from(&bytes[..]) {
                        let channel_name = channel.title().split('-').next().unwrap_or("News").trim();
                        for item in channel.items().iter().take(10) {
                            if let Some(title) = item.title() { 
                                headlines.push(format!("• {}: {}", channel_name, title)); 
                            }
                        }
                    }
                }
            }
        }
        let _ = tx.send(AppEvent::UpdateNews(format!("\n{}", headlines.join("\n\n"))));
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

pub struct NewsWidget<'a> { 
    pub text: &'a str, 
    pub is_selected: bool,
    pub tick_count: u64, // Added to track animation frames
}

impl<'a> Widget for NewsWidget<'a> { 
    fn render(self, area: Rect, buf: &mut Buffer) { 
        let block = standard_block(" Top News ", self.is_selected);
        let inner_area = block.inner(area);
        
        // Prevent divide-by-zero if the widget is squished
        let width = inner_area.width.max(1);
        
        // Estimate the total number of lines after wrapping
        let total_lines: u16 = self.text.lines()
            .map(|line| (line.chars().count() as u16 / width) + 1)
            .sum();

        // Calculate scroll offset based on tick_count
        let scroll_offset = if total_lines > inner_area.height {
            let speed = 15; // Increase this number to scroll SLOWER, decrease to scroll FASTER
            // Add a small buffer so it pauses on the last item before looping back to the top
            ((self.tick_count / speed) as u16) % (total_lines + 2)
        } else {
            0
        };

        Paragraph::new(self.text)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((scroll_offset, 0)) // Apply the vertical offset
            .render(area, buf); 
    } 
}