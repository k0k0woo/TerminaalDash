use crate::{app::{App, AppEvent, InputMode}, shared::standard_block};
use ratatui::{
    buffer::Buffer, layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style}, symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph, Widget, Wrap},
};
use std::{sync::mpsc, time::Duration};

#[derive(Clone, Default)]
pub struct StockOverviewData {
    pub symbol: String, pub company_name: String, pub price: f64,
    pub change: f64, pub change_percent: f64, pub market_cap: String, pub chart_data: Vec<f64>,
}

#[derive(Clone, Default)]
pub struct DetailedTickerData {
    pub symbol: String, pub company_name: String, pub description: String,
    pub industry: String, pub sector: String, pub price: f64, pub change: f64,
    pub change_percent: f64, pub market_cap: String, pub pe_ratio: String,
    pub analyst_rating: String, pub chart_data: Vec<f64>, pub is_loading: bool,
    pub related_tickers: Vec<String>, pub ticker_news: Vec<String>,
}

async fn get_yahoo_auth() -> Result<(reqwest::Client, String), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder().user_agent("Mozilla/5.0").cookie_store(true).build()?;
    let _ = client.get("https://fc.yahoo.com").send().await?;
    let crumb = client.get("https://query1.finance.yahoo.com/v1/test/getcrumb").send().await?.text().await?;
    Ok((client, crumb))
}

pub async fn fetch_stock_data(tx: mpsc::Sender<AppEvent>) {
    let client = reqwest::Client::builder().user_agent("Mozilla/5.0").build().unwrap_or_default();
    loop {
        let file_contents = tokio::fs::read_to_string("stocks.txt").await.unwrap_or_else(|_| "AAPL".to_string());
        let symbols: Vec<&str> = file_contents.lines().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        let mut ticker_parts = Vec::new();
        for symbol in &symbols {
            let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=1d", symbol);
            if let Ok(res) = client.get(&url).send().await {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if let Some(meta) = json["chart"]["result"][0]["meta"].as_object() {
                        let price = meta.get("regularMarketPrice").and_then(|p| p.as_f64()).unwrap_or(0.0);
                        let prev_close = meta.get("chartPreviousClose").and_then(|p| p.as_f64()).unwrap_or(price);
                        let change_percent = if prev_close > 0.0 { ((price - prev_close) / prev_close) * 100.0 } else { 0.0 };
                        let sign = if change_percent >= 0.0 { "+" } else { "" };
                        ticker_parts.push(format!("{} ${:.2} ({}{:.2}%)", symbol, price, sign, change_percent));
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }
        if !ticker_parts.is_empty() {
            let _ = tx.send(AppEvent::UpdateStock(format!("   •   {}   •   ", ticker_parts.join("   •   "))));
        }
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}


pub async fn fetch_stock_overview_data(tx: mpsc::Sender<AppEvent>) {
    loop {
        if let Ok((client, crumb)) = get_yahoo_auth().await {
            let file_contents = tokio::fs::read_to_string("stocks.txt").await.unwrap_or_else(|_| "AAPL".to_string());
            let symbols: Vec<&str> = file_contents.lines().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            
            if symbols.is_empty() { 
                tokio::time::sleep(Duration::from_secs(60)).await; 
                continue; 
            }

            for symbol in symbols {
                let mut overview = StockOverviewData { symbol: symbol.to_string(), company_name: symbol.to_string(), ..Default::default() };
                
                let chart_url = format!(
                    "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=15m&range=5d&crumb={}", 
                    symbol, crumb
                );

                if let Ok(res) = client.get(&chart_url).send().await {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(close_prices) = json["chart"]["result"][0]["indicators"]["quote"][0]["close"].as_array() {
                            let mut recent_prices: Vec<f64> = close_prices.iter().filter_map(|v| v.as_f64()).collect();
                            if recent_prices.len() > 50 { recent_prices = recent_prices[recent_prices.len() - 50..].to_vec(); }
                            overview.chart_data = recent_prices;
                        }
                        if let Some(meta) = json["chart"]["result"][0]["meta"].as_object() {
                            if let Some(name) = meta.get("shortName").and_then(|n| n.as_str()) { overview.company_name = name.to_string(); }
                            let price = meta.get("regularMarketPrice").and_then(|p| p.as_f64()).unwrap_or(0.0);
                            let prev_close = meta.get("chartPreviousClose").and_then(|p| p.as_f64()).unwrap_or(price);
                            overview.price = price; 
                            overview.change = price - prev_close;
                            if prev_close > 0.0 { overview.change_percent = (overview.change / prev_close) * 100.0; }
                        }
                    }
                }
                let _ = tx.send(AppEvent::UpdateStockOverview(overview));
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        } else {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}
pub async fn fetch_detailed_ticker(symbol: String, tx: mpsc::Sender<AppEvent>) {
    let mut data = DetailedTickerData {
        symbol: symbol.to_uppercase(),
        is_loading: false,
        ..Default::default()
    };

    if let Ok((client, crumb)) = get_yahoo_auth().await {
        // 1. Primary Data
        let summary_url = format!(
            "https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=assetProfile,summaryDetail,financialData&crumb={}", 
            symbol, crumb
        );
        
        // 2. NEW: Dedicated News Endpoint (Much more reliable)
        let news_url = format!(
            "https://query2.finance.yahoo.com/v1/finance/search?q={}&newsCount=3", 
            symbol
        );

        // Fetch Summary (Profile/Financials)
        if let Ok(res) = client.get(&summary_url).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(result) = json["quoteSummary"]["result"].as_array().and_then(|arr| arr.get(0)) {
                    if let Some(profile) = result.get("assetProfile") {
                        data.description = profile["longBusinessSummary"].as_str().unwrap_or("No description.").to_string();
                        data.industry = profile["industry"].as_str().unwrap_or("N/A").to_string();
                        data.sector = profile["sector"].as_str().unwrap_or("N/A").to_string();
                    }
                    if let Some(fin) = result.get("financialData") {
                        data.price = fin["currentPrice"]["raw"].as_f64().unwrap_or(0.0);
                        data.analyst_rating = fin["recommendationKey"].as_str().unwrap_or("N/A").to_string().replace('_', " ").to_uppercase();
                    }
                    if let Some(summary) = result.get("summaryDetail") {
                        data.market_cap = summary["marketCap"]["fmt"].as_str().unwrap_or("N/A").to_string();
                        data.pe_ratio = summary["forwardPE"]["fmt"].as_str().unwrap_or("N/A").to_string();
                    }
                }
            }
        }

        // Fetch News from Search Endpoint
        if let Ok(res) = client.get(&news_url).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(news_items) = json["news"].as_array() {
                    data.ticker_news = news_items.iter()
                        .filter_map(|item| item["title"].as_str())
                        .map(|s| s.to_string())
                        .collect();
                }
            }
        }

        // 3. Related Tickers (v6)
        let rec_url = format!("https://query2.finance.yahoo.com/v6/finance/recommendationsbysymbol/{}?crumb={}", symbol, crumb);
        if let Ok(res) = client.get(&rec_url).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(recommended) = json["finance"]["result"][0]["recommendedSymbols"].as_array() {
                    data.related_tickers = recommended.iter().filter_map(|s| s["symbol"].as_str()).map(|s| s.to_string()).take(5).collect();
                }
            }
        }

        // 4. Chart Data
        let chart_url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=15m&range=5d&crumb={}", symbol, crumb);
        if let Ok(res) = client.get(&chart_url).header("User-Agent", "Mozilla/5.0").send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(close_prices) = json["chart"]["result"][0]["indicators"]["quote"][0]["close"].as_array() {
                    let mut recent_prices: Vec<f64> = close_prices.iter().filter_map(|v| v.as_f64()).collect();
                    if recent_prices.len() > 50 { recent_prices = recent_prices[recent_prices.len() - 50..].to_vec(); }
                    data.chart_data = recent_prices;
                }
                if let Some(meta) = json["chart"]["result"][0]["meta"].as_object() {
                    let price = meta.get("regularMarketPrice").and_then(|p| p.as_f64()).unwrap_or(data.price);
                    let prev_close = meta.get("chartPreviousClose").and_then(|p| p.as_f64()).unwrap_or(price);
                    data.change = price - prev_close;
                    if prev_close > 0.0 { data.change_percent = (data.change / prev_close) * 100.0; }
                }
            }
        }
    }
    let _ = tx.send(AppEvent::UpdateDetailedTicker(data));
}

pub struct StockWidget<'a> { pub text: &'a str, pub tick_count: u64, pub is_selected: bool }
impl<'a> Widget for StockWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = standard_block(" Stocks ", self.is_selected);
        let inner_area = block.inner(area);
        block.render(area, buf);
        let inner_width = inner_area.width as usize;
        if inner_width == 0 { return; }
        
        let gap = "     •     ";
        let full_text = format!("{}{}", self.text, gap);
        let styled_spans = self.parse_ticker_with_colors(&full_text);
        
        let mut all_chars: Vec<(char, Style)> = Vec::new();
        for span in styled_spans { for c in span.content.chars() { all_chars.push((c, span.style)); } }
        let char_count = all_chars.len();
        if char_count == 0 { return; }
        
        let offset = if char_count > inner_width { (self.tick_count as usize / 2) % char_count } else { 0 };
        let visible_chars = all_chars.iter().cycle().skip(offset).take(inner_width);
        
        let mut final_spans = Vec::new();
        for (c, style) in visible_chars { final_spans.push(Span::styled(c.to_string(), *style)); }
        Paragraph::new(Line::from(final_spans)).render(inner_area, buf);
    }
}

impl<'a> StockWidget<'a> {
    fn parse_ticker_with_colors(&self, text: &str) -> Vec<Span<'static>> {
        let mut spans = Vec::new(); let mut current_pos = 0;
        while let Some(start_idx) = text[current_pos..].find('(') {
            let absolute_start = current_pos + start_idx;
            if let Some(end_idx) = text[absolute_start..].find(')') {
                let absolute_end = absolute_start + end_idx;
                spans.push(Span::raw(text[current_pos..absolute_start].to_string()));
                let pct_str = &text[absolute_start..=absolute_end];
                let numeric_part = pct_str.trim_matches(|c| c == '(' || c == ')' || c == '%' || c == '+');
                let val: f64 = numeric_part.parse().unwrap_or(0.0);
                let style = if val.abs() >= 0.5 { if val > 0.0 { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red) } } else { Style::default() };
                spans.push(Span::styled(pct_str.to_string(), style));
                current_pos = absolute_end + 1;
            } else { break; }
        }
        if current_pos < text.len() { spans.push(Span::raw(text[current_pos..].to_string())); }
        spans
    }
}


pub struct StockOverviewWidget<'a> { 
    pub data_list: &'a [StockOverviewData], 
    pub tick_count: u64,
    pub is_selected: bool 
}

impl<'a> Widget for StockOverviewWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = standard_block(" (5d) Mini Charts ", self.is_selected);
        let inner_area = block.inner(area);
        block.render(area, buf);
        
        if self.data_list.is_empty() {
            Paragraph::new("Loading Stocks...").alignment(ratatui::layout::Alignment::Center).render(inner_area, buf);
            return;
        }

        // --- LOOP LOGIC ---
        let list_len = self.data_list.len();
        if list_len == 0 { return; } // Guard against empty list

        // Calculate index: (Total Ticks / Ticks per Slide) % Number of items
        let display_index = (self.tick_count as usize / 250) % list_len;
        let data = &self.data_list[display_index];
        // ------------------

        if inner_area.height < 3 || inner_area.width < 5 { return; }
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(inner_area);

        let color = if data.change >= 0.0 { Color::Green } else { Color::Red };
        let sign = if data.change >= 0.0 { "+" } else { "" };
        
        // Show progress dots so you know there are more stocks
        let pager: String = self.data_list
            .iter()
            .enumerate()
            .map(|(i, _)| if i == display_index { "●" } else { "·" })
            .collect();

        let header_text = format!(
            "{} ({}) \n${:.2} | {}{:.2} ({}{:.2}%) {}",
            data.symbol, data.company_name,
            data.price, sign, data.change, sign, data.change_percent,pager
        );

        Paragraph::new(Span::styled(header_text, Style::default().fg(color))).render(chunks[0], buf);

        if !data.chart_data.is_empty() {
            let data_points: Vec<(f64, f64)> = data.chart_data.iter().enumerate().map(|(i, &p)| (i as f64, p)).collect();
            let min_y = data.chart_data.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_y = data.chart_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let max_x = (data_points.len().saturating_sub(1)) as f64;
            
            let dataset = Dataset::default()
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(color))
                .graph_type(GraphType::Line)
                .data(&data_points);

            Chart::new(vec![dataset])
                .x_axis(ratatui::widgets::Axis::default().bounds([0.0, max_x]))
                .y_axis(ratatui::widgets::Axis::default().bounds([min_y, max_y]))
                .render(chunks[1], buf);
        }
    }
}

pub struct FocusedStockOverviewWidget<'a> { pub app: &'a App }

impl<'a> Widget for FocusedStockOverviewWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = standard_block(" Advanced Ticker Terminal (Yahoo Finance Data) ", true);
        let inner = block.inner(area);
        block.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(inner);

        // 1. Search Bar
        let search_text = match self.app.input_mode {
            InputMode::SearchingTicker => format!(" Search Ticker: {}█", self.app.input_buffer),
            _ => " Keys: [s] or [/] to Search  |  [Esc] to Dashboard".to_string(),
        };
        Paragraph::new(search_text)
            .block(Block::default().borders(Borders::BOTTOM))
            .style(Style::default().fg(Color::Yellow))
            .render(layout[0], buf);

        let data = &self.app.detailed_ticker_data;
        
        if data.is_loading {
            Paragraph::new("\n\nFetching intelligence from Yahoo Finance...\nPlease Wait.")
                .alignment(ratatui::layout::Alignment::Center)
                .render(layout[1], buf);
            return;
        }

        if data.symbol.is_empty() {
            Paragraph::new("\n\nPress 's' to search for a ticker (e.g., AAPL, TSLA, MSFT).")
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(Color::DarkGray))
                .render(layout[1], buf);
            return;
        }

        let data_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[1]);

        let top_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(data_layout[0]);

        // --- LEFT PANE: Profile ---
        let overview_text = vec![
            Line::from(vec![
                Span::styled(format!(" {} ", data.symbol), Style::default().bg(Color::White).fg(Color::Black).add_modifier(Modifier::BOLD)),
                Span::raw(format!("  {} • {}", data.sector, data.industry)),
            ]),
            Line::from(""),
            Line::from(Span::styled("Company Summary:", Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED))),
            Line::from(data.description.clone()),
        ];
        
        Paragraph::new(overview_text)
            .wrap(Wrap { trim: false })
            .block(Block::default().padding(ratatui::widgets::Padding::new(1, 2, 1, 1)))
            .render(top_layout[0], buf);

        // --- RIGHT PANE: Financials, Peers, & News ---
        let right_col_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // Financials
                Constraint::Length(7), // Peers
                Constraint::Min(0),    // News
            ])
            .split(top_layout[1]);

        // Financials
        let rating_color = match data.analyst_rating.as_str() {
            "STRONG BUY" | "BUY" => Color::LightGreen,
            "SELL" | "STRONG SELL" => Color::LightRed,
            _ => Color::Yellow,
        };
        let financial_lines = vec![
            ListItem::new(Line::from(vec![Span::raw("Price:    "), Span::styled(format!("${:.2}", data.price), Style::default().fg(Color::White))])),
            ListItem::new(Line::from(vec![Span::raw("Mkt Cap:  "), Span::styled(data.market_cap.clone(), Style::default().fg(Color::White))])),
            ListItem::new(Line::from(vec![Span::raw("P/E:      "), Span::styled(data.pe_ratio.clone(), Style::default().fg(Color::White))])),
            ListItem::new(Line::from(vec![Span::raw("Rating:   "), Span::styled(data.analyst_rating.clone(), Style::default().fg(rating_color).add_modifier(Modifier::BOLD))])),
        ];
        List::new(financial_lines)
            .block(Block::default().title(" Financials ").borders(Borders::LEFT).padding(ratatui::widgets::Padding::new(2, 1, 0, 0)))
            .render(right_col_split[0], buf);

        // Peers
        let related_items: Vec<ListItem> = data.related_tickers.iter().map(|t| {
            ListItem::new(Line::from(vec![Span::styled(" ➜ ", Style::default().fg(Color::Yellow)), Span::raw(t.clone())]))
        }).collect();
        List::new(related_items)
            .block(Block::default().title(" Market Peers ").borders(Borders::LEFT).padding(ratatui::widgets::Padding::new(2, 1, 0, 0)))
            .render(right_col_split[1], buf);

        // --- Latest News (Using Paragraph for wrapping support) ---
        if data.ticker_news.is_empty() {
            Paragraph::new("No recent news found for this ticker.")
                .block(Block::default().title(" Latest News ").borders(Borders::LEFT).padding(ratatui::widgets::Padding::new(2, 1, 0, 0)))
                .style(Style::default().fg(Color::DarkGray))
                .render(right_col_split[2], buf);
        } else {
            let mut news_lines = Vec::new();
            for headline in &data.ticker_news {
                news_lines.push(Line::from(vec![
                    Span::styled(" • ", Style::default().fg(Color::Cyan)),
                    Span::raw(headline.clone()),
                ]));
                news_lines.push(Line::from(""));
            }
            Paragraph::new(news_lines)
                .block(Block::default().title(" Latest News ").borders(Borders::LEFT).padding(ratatui::widgets::Padding::new(2, 1, 0, 0)))
                .wrap(Wrap { trim: true })
                .render(right_col_split[2], buf);
        }
        // --- BOTTOM PANE: Chart ---
        let chart_color = if data.change >= 0.0 { Color::Green } else { Color::Red };
        let sign = if data.change >= 0.0 { "+" } else { "" };
        let chart_title = format!(" 5-Day Trend: ${:.2} ({}{:.2}%) ", data.price, sign, data.change_percent);

        if !data.chart_data.is_empty() {
            let data_points: Vec<(f64, f64)> = data.chart_data.iter().enumerate().map(|(i, &p)| (i as f64, p)).collect();
            let min_y = data.chart_data.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_y = data.chart_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let max_x = (data_points.len().saturating_sub(1)) as f64;

            let dataset = Dataset::default()
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(chart_color))
                .graph_type(GraphType::Line)
                .data(&data_points);

            Chart::new(vec![dataset])
                .block(Block::default().title(chart_title).borders(Borders::TOP).padding(ratatui::widgets::Padding::new(1, 1, 1, 0)))
                .x_axis(ratatui::widgets::Axis::default().bounds([0.0, max_x]))
                .y_axis(ratatui::widgets::Axis::default().bounds([min_y, max_y]).labels(vec![
                    Span::raw(format!("{:.2}", min_y)),
                    Span::raw(format!("{:.2}", max_y)),
                ]))
                .render(data_layout[1], buf);
        } else {
            Paragraph::new("Loading market data...")
                .block(Block::default().title(chart_title).borders(Borders::TOP))
                .alignment(ratatui::layout::Alignment::Center)
                .render(data_layout[1], buf);
        }
    }
}