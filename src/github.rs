use crate::{app::AppEvent, shared::standard_block};
use ratatui::{buffer::Buffer, layout::Rect, widgets::{Paragraph, Widget}};
use std::{sync::mpsc, time::Duration};

pub async fn fetch_github_data(tx: mpsc::Sender<AppEvent>) {
    dotenvy::dotenv().ok();
    let username = std::env::var("GITHUB_USERNAME").unwrap_or_else(|_| "torvalds".to_string());
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let client = reqwest::Client::builder().user_agent("Ratatui-Dashboard").build().unwrap_or_default();
    let url = format!("https://api.github.com/users/{}/events", username);
    loop {
        let mut req = client.get(&url);
        if !token.is_empty() { req = req.header("Authorization", format!("Bearer {}", token)); }
        if let Ok(res) = req.send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(events) = json.as_array() {
                    let mut lines = Vec::new();
                    for event in events.iter().take(3) {
                        let repo = event.get("repo").and_then(|r| r.get("name")).and_then(|v| v.as_str()).unwrap_or("?");
                        lines.push(format!("🚀 {}", repo));
                    }
                    let _ = tx.send(AppEvent::UpdateGithub(format!("\n{}", lines.join("\n"))));
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

pub struct GithubWidget<'a> { pub text: &'a str, pub is_selected: bool }
impl<'a> Widget for GithubWidget<'a> { 
    fn render(self, area: Rect, buf: &mut Buffer) { Paragraph::new(self.text).block(standard_block(" GitHub Updates ", self.is_selected)).render(area, buf); } 
}