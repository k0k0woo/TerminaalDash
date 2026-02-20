use ratatui::{
    backend::{Backend, CrosstermBackend},
    buffer::Buffer,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    widgets::{Block, BorderType, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph, Widget},
    Frame, Terminal,
};
use rand::{Rng, RngExt};
use chrono::{Datelike, Utc, Local, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io, time::Duration, sync::mpsc};

// --- DATA STRUCTURES ---

#[derive(Serialize, Deserialize, Clone)]
struct ScheduleItem {
    time: String,
    activity: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ReminderItem {
    task: String,
    is_done: bool,
}

#[derive(Clone, Default)]
struct StockOverviewData {
    symbol: String,
    company_name: String,
    price: f64,
    change: f64,
    change_percent: f64,
    market_cap: String,
    chart_data: Vec<f64>,
}

enum AppEvent {
    UpdateStock(String),
    UpdateWeather(String, WeatherCondition),
    UpdateTime(String),
    UpdateGithub(String),
    UpdateNews(String),
    UpdateStockOverview(StockOverviewData),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum WeatherCondition {
    Clear,
    Clouds,
    Rain,
    Storm,
    Snow,
    Unknown,
}

enum Action {
    Tick,
    Input(Event),
    Backend(AppEvent),
    Quit,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum FocusedScreen {
    Dashboard, Time, Stock, Weather, StockOverview, Github, News, Schedule, Reminders,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum SelectableWidget {
    Stock, Weather, StockOverview, Github, News, Schedule, Reminders,
}

struct App {
    focused_screen: FocusedScreen,
    selected_widget: SelectableWidget,
    tick_count: u64,
    stock_text: String, 
    weather_text: String,
    weather_condition: WeatherCondition,
    time_text: String,
    github_text: String,
    news_text: String,
    stock_overview_data: StockOverviewData,
    schedule: Vec<ScheduleItem>,
    reminders: Vec<ReminderItem>,
    reminder_index: usize,
    time_rect: Rect, stock_rect: Rect, weather_rect: Rect, stock_overview_rect: Rect, github_rect: Rect,
    news_rect: Rect, schedule_rect: Rect, reminders_rect: Rect,
}

impl App {
    fn new() -> Self {
        let schedule = serde_json::from_str(&std::fs::read_to_string("schedule.json").unwrap_or_else(|_| "[]".to_string())).unwrap_or_default();
        let reminders = serde_json::from_str(&std::fs::read_to_string("reminders.json").unwrap_or_else(|_| "[]".to_string())).unwrap_or_default();
        
        let mut default_overview = StockOverviewData::default();
        default_overview.symbol = "Loading...".to_string();

        Self {
            focused_screen: FocusedScreen::Dashboard,
            selected_widget: SelectableWidget::Stock,
            tick_count: 0,
            stock_text: "Initializing Smart Fetcher...".to_string(), 
            weather_text: "\nLoading Weather...".to_string(),
            weather_condition: WeatherCondition::Unknown,
            time_text: "Loading...".to_string(),
            github_text: "\nLoading GitHub...".to_string(),
            news_text: "\nLoading RSS Feeds...".to_string(),
            stock_overview_data: default_overview,
            schedule,
            reminders,
            reminder_index: 0,
            time_rect: Rect::default(), stock_rect: Rect::default(), weather_rect: Rect::default(),
            stock_overview_rect: Rect::default(), github_rect: Rect::default(), news_rect: Rect::default(), 
            schedule_rect: Rect::default(), reminders_rect: Rect::default(),
        }
    }

    fn save_reminders(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.reminders) {
            let _ = std::fs::write("reminders.json", json);
        }
    }

    fn toggle_reminder(&mut self) {
        if !self.reminders.is_empty() {
            self.reminders[self.reminder_index].is_done = !self.reminders[self.reminder_index].is_done;
            self.save_reminders();
        }
    }

    fn handle_click(&mut self, x: u16, y: u16) {
        if self.focused_screen != FocusedScreen::Dashboard {
            self.focused_screen = FocusedScreen::Dashboard;
            return;
        }
        let in_rect = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        if in_rect(self.time_rect) { self.focused_screen = FocusedScreen::Time; }
        else if in_rect(self.stock_rect) { self.focused_screen = FocusedScreen::Stock; }
        else if in_rect(self.weather_rect) { self.focused_screen = FocusedScreen::Weather; }
        else if in_rect(self.stock_overview_rect) { self.focused_screen = FocusedScreen::StockOverview; }
        else if in_rect(self.github_rect) { self.focused_screen = FocusedScreen::Github; }
        else if in_rect(self.news_rect) { self.focused_screen = FocusedScreen::News; }
        else if in_rect(self.schedule_rect) { self.focused_screen = FocusedScreen::Schedule; }
        else if in_rect(self.reminders_rect) { self.focused_screen = FocusedScreen::Reminders; }
    }

    fn move_selection(&mut self, key: KeyCode) {
        use SelectableWidget::*;
        match key {
            KeyCode::Up => {
                if self.selected_widget == Reminders && self.reminder_index > 0 {
                    self.reminder_index -= 1;
                } else {
                    self.selected_widget = match self.selected_widget {
                        Stock => Stock, Weather => Stock, StockOverview => Weather, Github => StockOverview,
                        News => Stock, Schedule => News, Reminders => Schedule,
                    }
                }
            }
            KeyCode::Down => {
                if self.selected_widget == Reminders && self.reminder_index < self.reminders.len().saturating_sub(1) {
                    self.reminder_index += 1;
                } else {
                    self.selected_widget = match self.selected_widget {
                        Stock => News, Weather => StockOverview, StockOverview => Github, Github => Github,
                        News => Schedule, Schedule => Reminders, Reminders => Reminders,
                    }
                }
            }
            KeyCode::Left => {
                self.selected_widget = match self.selected_widget {
                    Stock => Stock, Weather => Weather, StockOverview => StockOverview, Github => Github,
                    News => Weather, Schedule => StockOverview, Reminders => Github,
                }
            }
            KeyCode::Right => {
                self.selected_widget = match self.selected_widget {
                    Stock => Stock, Weather => News, StockOverview => Schedule, Github => Reminders,
                    News => News, Schedule => Schedule, Reminders => Reminders,
                }
            }
            _ => {}
        }
    }
}

// --- BACKGROUND FETCHERS ---

async fn fetch_time(tx: mpsc::Sender<AppEvent>) {
    loop {
        let now = Local::now();
        let formatted = now.format("%d %b %Y • %H:%M:%S ").to_string();
        let _ = tx.send(AppEvent::UpdateTime(formatted));
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn fetch_weather_data(tx: mpsc::Sender<AppEvent>) {
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
                        
                        // Parse weather condition for our animations
                        let desc_lower = desc.to_lowercase();
                        
                        let condition = if desc_lower.contains("storm") || desc_lower.contains("thunderstorm") {
                            WeatherCondition::Storm
                        } else if desc_lower.contains("snow") {
                            WeatherCondition::Snow
                        } else if desc_lower.contains("rain") || desc_lower.contains("drizzle") {
                            WeatherCondition::Rain
                        } else if desc_lower.contains("cloud") {
                            WeatherCondition::Clouds
                        } else {
                            WeatherCondition::Clear
                        };

                        let emoji = match condition {
                            WeatherCondition::Storm => "🌩️",
                            WeatherCondition::Snow => "❄️",
                            WeatherCondition::Rain => "🌧️",
                            WeatherCondition::Clouds => "☁️",
                            WeatherCondition::Clear => "☀️",
                            WeatherCondition::Unknown => "🌡️",
                        };

                        let formatted = format!("{}\n{} {:.1}°C\n{}", json.get("name").and_then(|v| v.as_str()).unwrap_or(&city), emoji, temp, desc);
                        
                        // Send both the text and the condition
                        let _ = tx.send(AppEvent::UpdateWeather(formatted, condition));
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(900)).await;
    }
}

async fn fetch_news_data(tx: mpsc::Sender<AppEvent>) {
    let feeds = vec!["https://www.investing.com/rss/news.rss", "https://news.ycombinator.com/rss", "http://feeds.bbci.co.uk/news/rss.xml", "https://finance.yahoo.com/news/rssindex"];
    let client = reqwest::Client::new();
    loop {
        let mut headlines = Vec::new();
        for url in &feeds {
            if let Ok(res) = client.get(*url).send().await {
                if let Ok(bytes) = res.bytes().await {
                    if let Ok(channel) = rss::Channel::read_from(&bytes[..]) {
                        for item in channel.items().iter().take(2) {
                            if let Some(title) = item.title() { headlines.push(format!("• {}", title)); }
                        }
                    }
                }
            }
        }
        let _ = tx.send(AppEvent::UpdateNews(format!("\n{}", headlines.join("\n"))));
        tokio::time::sleep(Duration::from_secs(1200)).await;
    }
}

async fn fetch_github_data(tx: mpsc::Sender<AppEvent>) {
    dotenvy::dotenv().ok();
    let username = std::env::var("GITHUB_USERNAME").unwrap_or_else(|_| "torvalds".to_string());
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let client = reqwest::Client::new();
    let url = format!("https://api.github.com/users/{}/events", username);
    loop {
        let mut req = client.get(&url).header("User-Agent", "Ratatui-Dashboard");
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

async fn fetch_stock_data(tx: mpsc::Sender<AppEvent>) {
    // Mimic a standard web browser to bypass bot protection
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .build()
        .unwrap_or_default();

    loop {
        let file_contents = tokio::fs::read_to_string("stocks.txt").await.unwrap_or_else(|_| "AAPL".to_string());
        let symbols: Vec<&str> = file_contents.lines().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        
        let mut ticker_parts = Vec::new();
        
        for symbol in &symbols {
            // Use the v8 chart endpoint to pull just 1 day of data
            let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=1d", symbol);
            
            if let Ok(res) = client.get(&url).send().await {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    // Extract the live price from the meta block just like the overview widget
                    if let Some(meta) = json["chart"]["result"][0]["meta"].as_object() {
                        let price = meta.get("regularMarketPrice").and_then(|p| p.as_f64()).unwrap_or(0.0);
                        let prev_close = meta.get("chartPreviousClose").and_then(|p| p.as_f64()).unwrap_or(price);
                        
                        // Calculate change so we can add a nice +/- percentage to the tape
                        let change_percent = if prev_close > 0.0 {
                            ((price - prev_close) / prev_close) * 100.0
                        } else {
                            0.0
                        };
                        
                        let sign = if change_percent >= 0.0 { "+" } else { "" };
                        
                        // Formats as: AAPL $260.58 (+1.54%)
                        ticker_parts.push(format!("{} ${:.2} ({}{:.2}%)", symbol, price, sign, change_percent));
                    }
                }
            }
            // Sleep briefly to avoid getting rate-limited by Yahoo
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }
        
        if !ticker_parts.is_empty() {
            let _ = tx.send(AppEvent::UpdateStock(format!("   •   {}   •   ", ticker_parts.join("   •   "))));
        }
        
        // Wait 5 minutes before refreshing the entire tape
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}
async fn fetch_stock_overview_data(tx: mpsc::Sender<AppEvent>) {
    // Mimic a standard web browser to keep Yahoo happy
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .build()
        .unwrap_or_default();

    loop {
        let file_contents = tokio::fs::read_to_string("stocks.txt").await.unwrap_or_else(|_| "AAPL".to_string());
        let symbols: Vec<&str> = file_contents.lines().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        
        if symbols.is_empty() {
            tokio::time::sleep(Duration::from_secs(60)).await;
            continue;
        }

        for symbol in symbols {
            let mut overview = StockOverviewData {
                symbol: symbol.to_string(),
                company_name: symbol.to_string(),
                ..Default::default()
            };

            // Rely entirely on the v8 chart endpoint, which we know works
            let chart_url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=15m&range=5d", symbol);
            
            if let Ok(res) = client.get(&chart_url).send().await {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    
                    // 1. Extract the Chart Data
                    if let Some(close_prices) = json["chart"]["result"][0]["indicators"]["quote"][0]["close"].as_array() {
                        let mut recent_prices: Vec<f64> = close_prices.iter()
                            .filter_map(|v| v.as_f64())
                            .collect();
                        
                        if recent_prices.len() > 50 {
                            recent_prices = recent_prices[recent_prices.len() - 50..].to_vec();
                        }
                        overview.chart_data = recent_prices;
                    }

                    // 2. Extract the Stats from the chart's "meta" block
                    if let Some(meta) = json["chart"]["result"][0]["meta"].as_object() {
                        if let Some(name) = meta.get("shortName").and_then(|n| n.as_str()) {
                            overview.company_name = name.to_string();
                        }
                        
                        let price = meta.get("regularMarketPrice").and_then(|p| p.as_f64()).unwrap_or(0.0);
                        let prev_close = meta.get("chartPreviousClose").and_then(|p| p.as_f64()).unwrap_or(price);
                        
                        overview.price = price;
                        overview.change = price - prev_close;
                        if prev_close > 0.0 {
                            overview.change_percent = (overview.change / prev_close) * 100.0;
                        }
                        overview.market_cap = "N/A".to_string(); // We won't use this anymore
                    }
                }
            }

            let _ = tx.send(AppEvent::UpdateStockOverview(overview));
            
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
}

// --- MAIN LOOP ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, rx) = mpsc::channel();
    let tx_w = tx.clone(); let tx_t = tx.clone(); let tx_g = tx.clone(); let tx_n = tx.clone(); let tx_so = tx.clone();
    
    tokio::spawn(async move { fetch_stock_data(tx).await; });
    tokio::spawn(async move { fetch_weather_data(tx_w).await; });
    tokio::spawn(async move { fetch_time(tx_t).await; });
    tokio::spawn(async move { fetch_github_data(tx_g).await; });
    tokio::spawn(async move { fetch_news_data(tx_n).await; });
    tokio::spawn(async move { fetch_stock_overview_data(tx_so).await; }); // Spawn new fetcher

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app, rx);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    if let Err(err) = res { println!("{:?}", err); }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App, rx: mpsc::Receiver<AppEvent>) -> io::Result<()> {
    let (action_tx, action_rx) = mpsc::channel();

    let tx_input = action_tx.clone();
    std::thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(16)).unwrap_or(false) {
                if let Ok(evt) = event::read() {
                    if tx_input.send(Action::Input(evt)).is_err() { break; }
                }
            }
        }
    });

    let tx_tick = action_tx.clone();
    std::thread::spawn(move || {
        loop {
            if tx_tick.send(Action::Tick).is_err() { break; }
            std::thread::sleep(Duration::from_millis(50));
        }
    });

    let tx_backend = action_tx.clone();
    std::thread::spawn(move || {
        while let Ok(evt) = rx.recv() {
            if tx_backend.send(Action::Backend(evt)).is_err() { break; }
        }
    });

    loop {
        terminal.draw(|f| ui(f, app));

        let Ok(action) = action_rx.recv() else { break };
        let mut should_quit = handle_action(app, action);

        while let Ok(pending_action) = action_rx.try_recv() {
            if handle_action(app, pending_action) {
                should_quit = true;
            }
        }

        if should_quit { return Ok(()); }
    }
    Ok(())
}

fn handle_action(app: &mut App, action: Action) -> bool {
    match action {
        Action::Tick => {
            app.tick_count = app.tick_count.wrapping_add(1);
        }
        Action::Backend(event) => match event {
            AppEvent::UpdateStock(s) => app.stock_text = s,
            AppEvent::UpdateWeather(s, c) => {
            app.weather_text = s;
            app.weather_condition = c;
            },
            AppEvent::UpdateTime(s) => app.time_text = s,
            AppEvent::UpdateGithub(s) => app.github_text = s,
            AppEvent::UpdateNews(s) => app.news_text = s,
            AppEvent::UpdateStockOverview(d) => app.stock_overview_data = d,
        },
        Action::Input(event) => match event {
            Event::Key(key) => {
                if key.code == KeyCode::Char('q') { return true; }
                if key.code == KeyCode::Esc { app.focused_screen = FocusedScreen::Dashboard; }
                if key.code == KeyCode::Char(' ') && app.selected_widget == SelectableWidget::Reminders { 
                    app.toggle_reminder(); 
                }
                if app.focused_screen == FocusedScreen::Dashboard {
                    match key.code {
                        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => app.move_selection(key.code),
                        KeyCode::Enter => {
                            app.focused_screen = match app.selected_widget {
                                SelectableWidget::Stock => FocusedScreen::Stock,
                                SelectableWidget::Weather => FocusedScreen::Weather,
                                SelectableWidget::StockOverview => FocusedScreen::StockOverview,
                                SelectableWidget::Github => FocusedScreen::Github,
                                SelectableWidget::News => FocusedScreen::News,
                                SelectableWidget::Schedule => FocusedScreen::Schedule,
                                SelectableWidget::Reminders => FocusedScreen::Reminders,
                            };
                        }
                        _ => {}
                    }
                }
            }
            Event::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) { 
                    app.handle_click(mouse.column, mouse.row); 
                }
            }
            _ => {}
        },
        Action::Quit => return true,
    }
    false
}

// --- SHARED UI HELPERS ---

fn standard_block<'a>(title: &'a str, is_selected: bool) -> Block<'a> {
    let style = if is_selected { Style::default().fg(Color::White) } else { Style::default().fg(Color::DarkGray) };
    let b_type = if is_selected { BorderType::Thick } else { BorderType::Rounded };
    Block::default().title(title).borders(Borders::ALL).border_type(b_type).border_style(style)
}

// --- COMPONENT WIDGETS ---

struct TimeWidget<'a> { text: &'a str }
impl<'a> Widget for TimeWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.text)
            .block(standard_block(" Time ", false))
            .alignment(ratatui::layout::Alignment::Center)
            .render(area, buf);
    }
}

struct StockWidget<'a> { text: &'a str, tick_count: u64, is_selected: bool }
impl<'a> Widget for StockWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_width = area.width.saturating_sub(2) as usize; 
        let char_count = self.text.chars().count();
        let display_ticker = if char_count > 0 && self.text.contains("   •   ") {
            let offset = (self.tick_count as usize / 4) % char_count;
            self.text.chars().cycle().skip(offset).take(inner_width).collect::<String>()
        } else {
            self.text.to_string()
        };
        Paragraph::new(display_ticker)
            .block(standard_block(" Stocks ", self.is_selected))
            .render(area, buf);
    }
}

struct WeatherWidget<'a> { 
    text: &'a str, 
    is_selected: bool,
    tick_count: u64,
    condition: WeatherCondition,
}

impl<'a> Widget for WeatherWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = standard_block(" Weather ", self.is_selected);
        let inner_area = block.inner(area);
        block.render(area, buf);

        // 1. Render the actual text first
        Paragraph::new(self.text)
            .alignment(ratatui::layout::Alignment::Center)
            .render(inner_area, buf);

        if inner_area.height == 0 || inner_area.width == 0 { return; }

        // 2. Overlay the animations on empty background spaces!
        match self.condition {
            WeatherCondition::Rain | WeatherCondition::Storm => {
                let is_storm = self.condition == WeatherCondition::Storm;
                let speed = if is_storm { 2 } else { 1 };
                let symbol = if is_storm { "/" } else { "|" };
                let color = if is_storm { Color::Blue } else { Color::Cyan };
                
                for x in inner_area.left()..inner_area.right() {
                    let col_seed = (x as u64).wrapping_mul(1103515245);
                    if col_seed % 10 > 4 { continue; } // Rain drop density
                    
                    let drop_y = inner_area.top() + (((self.tick_count * speed) + col_seed % 100) % inner_area.height as u64) as u16;
                    
                    if let Some(cell) = buf.cell_mut((x, drop_y)) {
                        if cell.symbol() == " " { // Protect the text!
                            cell.set_symbol(symbol).set_fg(color);
                        }
                    }
                }
                
                // Lightning flashes for storms
                if is_storm && self.tick_count % 80 < 2 {
                    for y in inner_area.top()..inner_area.bottom() {
                        for x in inner_area.left()..inner_area.right() {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                if cell.symbol() == " " && (x + y) % 3 == 0 {
                                    cell.set_bg(Color::DarkGray);
                                }
                            }
                        }
                    }
                }
            }
            WeatherCondition::Snow => {
                for x in inner_area.left()..inner_area.right() {
                    let col_seed = (x as u64).wrapping_mul(123456789);
                    if col_seed % 10 > 2 { continue; } // Snow density
                    
                    let speed = 2; // Tick divisor (slower than rain)
                    let drop_y = inner_area.top() + (((self.tick_count / speed) + col_seed % 100) % inner_area.height as u64) as u16;
                    let drift = (self.tick_count / 4 + col_seed) % 3; // Swirl sideways
                    
                    let drop_x = (x + drift as u16).clamp(inner_area.left(), inner_area.right() - 1);
                    
                    if let Some(cell) = buf.cell_mut((drop_x, drop_y)) {
                        if cell.symbol() == " " {
                            cell.set_symbol("*").set_fg(Color::White);
                        }
                    }
                }
            }
            WeatherCondition::Clouds => {
                for y in inner_area.top()..inner_area.bottom() {
                    let row_seed = (y as u64).wrapping_mul(987654321);
                    if row_seed % 4 != 0 { continue; } // Space out the clouds
                    
                    let x_pos = inner_area.left() + (((self.tick_count / 4) + row_seed % 100) % inner_area.width as u64) as u16;
                    if let Some(cell) = buf.cell_mut((x_pos, y)) {
                        if cell.symbol() == " " {
                            cell.set_symbol("~").set_fg(Color::DarkGray);
                        }
                    }
                }
            }
            WeatherCondition::Clear => {
                // Subtle twinkling sunrays/stars
                for y in inner_area.top()..inner_area.bottom() {
                    for x in inner_area.left()..inner_area.right() {
                        let seed = (x as u64).wrapping_mul(111).wrapping_add(y as u64 * 222);
                        if seed % 40 == 0 && (self.tick_count + seed) % 40 < 20 {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                if cell.symbol() == " " {
                                    cell.set_symbol("+").set_fg(Color::Yellow);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// New Stock Overview Widget
struct StockOverviewWidget<'a> { data: &'a StockOverviewData, is_selected: bool }
impl<'a> Widget for StockOverviewWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = standard_block(" (5d) Mini Chart ", self.is_selected);
        let inner_area = block.inner(area);
        block.render(area, buf);

        if inner_area.height < 3 || inner_area.width < 5 { return; }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(inner_area);

        let color = if self.data.change >= 0.0 { Color::Green } else { Color::Red };
        let sign = if self.data.change >= 0.0 { "+" } else { "" };
        
        // Removed the Market Cap portion for a cleaner look
        let header_text = format!(
            "{} ({}) \n${:.2} | {}{:.2} ({}{:.2}%)",
            self.data.symbol, self.data.company_name, 
            self.data.price, sign, self.data.change, sign, self.data.change_percent
        );

        Paragraph::new(ratatui::text::Span::styled(header_text, Style::default().fg(color)))
            .render(chunks[0], buf);

        // Bottom Chart
        if !self.data.chart_data.is_empty() {
            let data_points: Vec<(f64, f64)> = self.data.chart_data
                .iter()
                .enumerate()
                .map(|(i, &p)| (i as f64, p))
                .collect();

            let min_y = self.data.chart_data.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_y = self.data.chart_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
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
        } else {
            Paragraph::new("Loading chart data...")
                .alignment(ratatui::layout::Alignment::Center)
                .render(chunks[1], buf);
        }
    }
}

struct GithubWidget<'a> { text: &'a str, is_selected: bool }
impl<'a> Widget for GithubWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.text)
            .block(standard_block(" GitHub Updates ", self.is_selected))
            .render(area, buf);
    }
}

struct NewsWidget<'a> { text: &'a str, is_selected: bool }
impl<'a> Widget for NewsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.text)
            .block(standard_block(" Top News ", self.is_selected))
            .wrap(ratatui::widgets::Wrap { trim: true })
            .render(area, buf);
    }
}

struct ScheduleWidget<'a> { schedule: &'a [ScheduleItem], is_selected: bool }
impl<'a> Widget for ScheduleWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self.schedule.iter().map(|i| {
            ListItem::new(format!(" {} | {}", i.time, i.activity))
        }).collect();
        List::new(items)
            .block(standard_block(" Schedule ", self.is_selected))
            .render(area, buf);
    }
}

struct RemindersWidget<'a> { reminders: &'a [ReminderItem], active_idx: usize, is_selected: bool }
impl<'a> Widget for RemindersWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self.reminders.iter().enumerate().map(|(i, r)| {
            let sym = if r.is_done { "[x]" } else { "[ ]" };
            let style = if i == self.active_idx && self.is_selected { 
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) 
            } else { 
                Style::default() 
            };
            ListItem::new(format!(" {} {}", sym, r.task)).style(style)
        }).collect();
        List::new(items)
            .block(standard_block(" Reminders (Space) ", self.is_selected))
            .render(area, buf);
    }
}

struct FocusedViewWidget<'a> { title: &'a str }
impl<'a> Widget for FocusedViewWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("\n\nDetailed view goes here.\n\nPress 'Esc' or Click anywhere to return.")
            .block(standard_block(self.title, true))
            .alignment(ratatui::layout::Alignment::Center)
            .render(area, buf);
    }
}

struct MatrixEdgeOverlay { tick: u64 }
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
                        
                        if dist_to_head == 0 { cell.set_fg(Color::White); } 
                        else if dist_to_head < 3 { cell.set_fg(Color::LightGreen); } 
                        else if dist_to_head > 11 { cell.set_fg(Color::DarkGray); } 
                        else { cell.set_fg(Color::Green); }
                    }
                }
            }
        }
    }
}

// --- MAIN LAYOUT & RENDER LOGIC ---

fn ui(f: &mut Frame, app: &mut App) {
    let full_area = f.area();
    let safe_area = Rect {
        x: full_area.x.saturating_add(4), y: full_area.y.saturating_add(2),
        width: full_area.width.saturating_sub(8), height: full_area.height.saturating_sub(4),
    };
    let inner_area = if full_area.width > 20 && full_area.height > 10 { safe_area } else { full_area };

    if app.focused_screen != FocusedScreen::Dashboard {
        let title = format!(" Focused: {:?} ", app.focused_screen);
        f.render_widget(FocusedViewWidget { title: &title }, inner_area);
        f.render_widget(MatrixEdgeOverlay { tick: app.tick_count }, full_area);
        return;
    }

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner_area);

    let top_bar = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(main_layout[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(main_layout[1]);

    // NEW LEFT COLUMN LAYOUT
    let left_col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30), // Weather
            Constraint::Percentage(40), // Stock Chart Overview
            Constraint::Percentage(30), // GitHub
        ])
        .split(body[0]);

    let right_col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(30), Constraint::Percentage(30)])
        .split(body[1]);

    app.time_rect = top_bar[0]; app.stock_rect = top_bar[1];
    app.weather_rect = left_col[0]; app.stock_overview_rect = left_col[1]; app.github_rect = left_col[2];
    app.news_rect = right_col[0]; app.schedule_rect = right_col[1]; app.reminders_rect = right_col[2];

    let sel = app.selected_widget;

    f.render_widget(TimeWidget { text: &app.time_text }, app.time_rect);
    f.render_widget(StockWidget { text: &app.stock_text, tick_count: app.tick_count, is_selected: sel == SelectableWidget::Stock }, app.stock_rect);
    
    f.render_widget(WeatherWidget { 
        text: &app.weather_text, 
        is_selected: sel == SelectableWidget::Weather,
        tick_count: app.tick_count,
        condition: app.weather_condition
    }, app.weather_rect);
    f.render_widget(StockOverviewWidget { data: &app.stock_overview_data, is_selected: sel == SelectableWidget::StockOverview }, app.stock_overview_rect);
    f.render_widget(GithubWidget { text: &app.github_text, is_selected: sel == SelectableWidget::Github }, app.github_rect);
    
    f.render_widget(NewsWidget { text: &app.news_text, is_selected: sel == SelectableWidget::News }, app.news_rect);
    f.render_widget(ScheduleWidget { schedule: &app.schedule, is_selected: sel == SelectableWidget::Schedule }, app.schedule_rect);
    f.render_widget(RemindersWidget { reminders: &app.reminders, active_idx: app.reminder_index, is_selected: sel == SelectableWidget::Reminders }, app.reminders_rect);

    f.render_widget(MatrixEdgeOverlay { tick: app.tick_count }, full_area);
}