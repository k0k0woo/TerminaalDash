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
    widgets::{Block, BorderType, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph, Widget, Wrap},
    Frame, Terminal,
    text::{Line, Span}
};
use rand::{Rng, RngExt};
use chrono::Local;
use serde::{Deserialize, Serialize};
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

#[derive(Clone, Default)]
struct DetailedTickerData {
    symbol: String,
    company_name: String,
    description: String,
    industry: String,
    sector: String,
    price: f64,
    change: f64,          // Added for the chart title
    change_percent: f64,  // Added for the chart title
    market_cap: String,
    pe_ratio: String,
    analyst_rating: String,
    chart_data: Vec<f64>, // Added for the chart
    is_loading: bool,
    related_tickers: Vec<String>,
    ticker_news: Vec<String>,
}

enum AppEvent {
    UpdateStock(String),
    UpdateWeather(String, WeatherCondition),
    UpdateTime(String),
    UpdateGithub(String),
    UpdateNews(String),
    UpdateStockOverview(StockOverviewData),
    UpdateDetailedTicker(DetailedTickerData),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum WeatherCondition {
    Clear, Clouds, Rain, Storm, Snow, Unknown,
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

#[derive(Debug, PartialEq, Clone, Copy)]
enum InputMode {
    Normal,
    AddingReminder,
    AddingScheduleTime,
    AddingScheduleActivity,
    SearchingTicker,
}

struct App {
    focused_screen: FocusedScreen,
    selected_widget: SelectableWidget,
    input_mode: InputMode,
    input_buffer: String,
    temp_schedule_time: String,
    tick_count: u64,
    stock_text: String, 
    weather_text: String,
    weather_condition: WeatherCondition,
    time_text: String,
    github_text: String,
    news_text: String,
    stock_overview_list: Vec<StockOverviewData>, // UPDATED HERE
    detailed_ticker_data: DetailedTickerData,
    schedule: Vec<ScheduleItem>,
    schedule_index: usize,
    reminders: Vec<ReminderItem>,
    reminder_index: usize,
    time_rect: Rect, stock_rect: Rect, weather_rect: Rect, stock_overview_rect: Rect, github_rect: Rect,
    news_rect: Rect, schedule_rect: Rect, reminders_rect: Rect,
    tx: Option<mpsc::Sender<AppEvent>>, // Used to spawn on-demand tasks
}

fn load_json<T: Default + serde::de::DeserializeOwned>(path: &str) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

async fn get_yahoo_auth() -> Result<(reqwest::Client, String), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .cookie_store(true) 
        .build()?;
    let _ = client.get("https://fc.yahoo.com").send().await?;
    let crumb = client
        .get("https://query1.finance.yahoo.com/v1/test/getcrumb")
        .send()
        .await?
        .text()
        .await?;
    Ok((client, crumb))
}

impl App {
    fn new(tx: mpsc::Sender<AppEvent>) -> Self {
        let mut schedule: Vec<ScheduleItem> = load_json("schedule.json");
        schedule.sort_by(|a, b| a.time.cmp(&b.time));
        
        let reminders = load_json("reminders.json");

        Self {
            focused_screen: FocusedScreen::Dashboard,
            selected_widget: SelectableWidget::Stock,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            temp_schedule_time: String::new(),
            tick_count: 0,
            stock_text: "Initializing Smart Fetcher...".to_string(), 
            weather_text: "\nLoading Weather...".to_string(),
            weather_condition: WeatherCondition::Unknown,
            time_text: "Loading...".to_string(),
            github_text: "\nLoading GitHub...".to_string(),
            news_text: "\nLoading RSS Feeds...".to_string(),
            stock_overview_list: Vec::new(), // FIX 1: Initialized the list correctly
            detailed_ticker_data: DetailedTickerData::default(),
            schedule,
            schedule_index: 0,
            reminders,
            reminder_index: 0,
            time_rect: Rect::default(), stock_rect: Rect::default(), weather_rect: Rect::default(),
            stock_overview_rect: Rect::default(), github_rect: Rect::default(), news_rect: Rect::default(), 
            schedule_rect: Rect::default(), reminders_rect: Rect::default(),
            tx: Some(tx),
        }
    }

    fn save_reminders(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.reminders) {
            let _ = std::fs::write("reminders.json", json);
        }
    }

    fn save_schedule(&mut self) {
        self.schedule.sort_by(|a, b| a.time.cmp(&b.time)); 
        if let Ok(json) = serde_json::to_string_pretty(&self.schedule) {
            let _ = std::fs::write("schedule.json", json);
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
            self.input_mode = InputMode::Normal;
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
        tokio::time::sleep(Duration::from_secs(900)).await;
    }
}

async fn fetch_news_data(tx: mpsc::Sender<AppEvent>) {
    let feeds = vec![
        "https://www.investing.com/rss/news.rss",
        "https://news.ycombinator.com/rss",
        "https://feeds.bbci.co.uk/news/rss.xml"
    ];
    let client = reqwest::Client::builder().user_agent("Mozilla/5.0").build().unwrap_or_default();
    loop {
        let mut headlines = Vec::new();
        for url in &feeds {
            if let Ok(res) = client.get(*url).send().await {
                if let Ok(bytes) = res.bytes().await {
                    if let Ok(channel) = rss::Channel::read_from(&bytes[..]) {
                        let channel_name = channel.title().split('-').next().unwrap_or("News").trim();
                        for item in channel.items().iter().take(7) {
                            if let Some(title) = item.title() { 
                                headlines.push(format!("• {}: {}", channel_name, title)); 
                            }
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

async fn fetch_stock_data(tx: mpsc::Sender<AppEvent>) {
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

async fn fetch_stock_overview_data(tx: mpsc::Sender<AppEvent>) {
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
}async fn fetch_detailed_ticker(symbol: String, tx: mpsc::Sender<AppEvent>) {
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
    
    tokio::spawn(async move { fetch_stock_data(tx_w).await; }); 
    tokio::spawn(async move { fetch_weather_data(tx_t).await; });
    tokio::spawn(async move { fetch_time(tx_g).await; });
    tokio::spawn(async move { fetch_github_data(tx_n).await; });
    tokio::spawn(async move { fetch_news_data(tx_so).await; });
    
    let tx_so_loop = tx.clone();
    tokio::spawn(async move { fetch_stock_overview_data(tx_so_loop).await; });

    let mut app = App::new(tx.clone());
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
            if handle_action(app, pending_action) { should_quit = true; }
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
            AppEvent::UpdateStockOverview(d) => {
                if let Some(pos) = app.stock_overview_list.iter().position(|s| s.symbol == d.symbol) {
                    app.stock_overview_list[pos] = d;
                } else {
                    app.stock_overview_list.push(d);
                }
            },
            AppEvent::UpdateDetailedTicker(d) => {
                app.detailed_ticker_data = d;
            }
        },
        Action::Input(event) => match event {
            Event::Key(key) => {
                if key.code == KeyCode::Esc { 
                    app.focused_screen = FocusedScreen::Dashboard; 
                    app.input_mode = InputMode::Normal;
                    app.input_buffer.clear();
                    return false;
                }

                if app.input_mode != InputMode::Normal {
                    match key.code {
                        KeyCode::Enter => {
                            match app.input_mode {
                                InputMode::AddingReminder => {
                                    if !app.input_buffer.trim().is_empty() {
                                        app.reminders.push(ReminderItem { task: app.input_buffer.clone(), is_done: false });
                                        app.save_reminders();
                                    }
                                    app.input_mode = InputMode::Normal;
                                }
                                InputMode::AddingScheduleTime => {
                                    if !app.input_buffer.trim().is_empty() {
                                        app.temp_schedule_time = app.input_buffer.clone();
                                        app.input_mode = InputMode::AddingScheduleActivity;
                                    } else { app.input_mode = InputMode::Normal; }
                                }
                                InputMode::AddingScheduleActivity => {
                                    if !app.input_buffer.trim().is_empty() {
                                        app.schedule.push(ScheduleItem { 
                                            time: app.temp_schedule_time.clone(), 
                                            activity: app.input_buffer.clone() 
                                        });
                                        app.save_schedule();
                                    }
                                    app.input_mode = InputMode::Normal;
                                }
                                InputMode::SearchingTicker => {
                                    if !app.input_buffer.trim().is_empty() {
                                        if let Some(tx) = &app.tx {
                                            app.detailed_ticker_data.is_loading = true;
                                            let symbol = app.input_buffer.trim().to_string();
                                            let tx_clone = tx.clone();
                                            tokio::spawn(async move {
                                                fetch_detailed_ticker(symbol, tx_clone).await;
                                            });
                                        }
                                    }
                                    app.input_mode = InputMode::Normal;
                                }
                                _ => {}
                            }
                            app.input_buffer.clear();
                        }
                        KeyCode::Backspace => { app.input_buffer.pop(); }
                        KeyCode::Char(c) => { app.input_buffer.push(c); }
                        _ => {}
                    }
                    return false;
                }

                if key.code == KeyCode::Char('q') { return true; }
                
                match app.focused_screen {
                    FocusedScreen::Dashboard => {
                        if key.code == KeyCode::Char(' ') && app.selected_widget == SelectableWidget::Reminders { 
                            app.toggle_reminder(); 
                        }
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
                    FocusedScreen::StockOverview => {
                        if key.code == KeyCode::Char('s') || key.code == KeyCode::Char('/') {
                            app.input_mode = InputMode::SearchingTicker;
                            app.input_buffer.clear();
                        }
                    }
                    FocusedScreen::Reminders => {
                        match key.code {
                            KeyCode::Up => { if app.reminder_index > 0 { app.reminder_index -= 1; } }
                            KeyCode::Down => { if app.reminder_index < app.reminders.len().saturating_sub(1) { app.reminder_index += 1; } }
                            KeyCode::Char(' ') | KeyCode::Enter => { app.toggle_reminder(); }
                            KeyCode::Char('a') => { app.input_mode = InputMode::AddingReminder; app.input_buffer.clear(); }
                            KeyCode::Char('d') | KeyCode::Backspace => {
                                if !app.reminders.is_empty() {
                                    app.reminders.remove(app.reminder_index);
                                    if app.reminder_index >= app.reminders.len() { app.reminder_index = app.reminders.len().saturating_sub(1); }
                                    app.save_reminders();
                                }
                            }
                            _ => {}
                        }
                    }
                    FocusedScreen::Schedule => {
                        match key.code {
                            KeyCode::Up => { if app.schedule_index > 0 { app.schedule_index -= 1; } }
                            KeyCode::Down => { if app.schedule_index < app.schedule.len().saturating_sub(1) { app.schedule_index += 1; } }
                            KeyCode::Char('a') => { app.input_mode = InputMode::AddingScheduleTime; app.input_buffer.clear(); }
                            KeyCode::Char('d') | KeyCode::Backspace => {
                                if !app.schedule.is_empty() {
                                    app.schedule.remove(app.schedule_index);
                                    if app.schedule_index >= app.schedule.len() { app.schedule_index = app.schedule.len().saturating_sub(1); }
                                    app.save_schedule();
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
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

struct TimeWidget<'a> { text: &'a str }
impl<'a> Widget for TimeWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) { Paragraph::new(self.text).block(standard_block(" Time ", false)).alignment(ratatui::layout::Alignment::Center).render(area, buf); }
}

struct StockWidget<'a> { text: &'a str, tick_count: u64, is_selected: bool }
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
                        if cell.symbol() == " " {
                            cell.set_symbol(symbol).set_fg(color);
                        }
                    }
                }
                
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
                    if col_seed % 10 > 2 { continue; } 
                    
                    let speed = 2; 
                    let drop_y = inner_area.top() + (((self.tick_count / speed) + col_seed % 100) % inner_area.height as u64) as u16;
                    let drift = (self.tick_count / 4 + col_seed) % 3; 
                    
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
                    if row_seed % 4 != 0 { continue; } 
                    
                    let x_pos = inner_area.left() + (((self.tick_count / 2) + row_seed % 100) % inner_area.width as u64) as u16;
                    if let Some(cell) = buf.cell_mut((x_pos, y)) {
                        if cell.symbol() == " " {
                            cell.set_symbol("~").set_fg(Color::DarkGray);
                        }
                    }
                }
            }
            WeatherCondition::Clear => {
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

struct StockOverviewWidget<'a> { 
    data_list: &'a [StockOverviewData], 
    tick_count: u64,
    is_selected: bool 
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

struct GithubWidget<'a> { text: &'a str, is_selected: bool }
impl<'a> Widget for GithubWidget<'a> { fn render(self, area: Rect, buf: &mut Buffer) { Paragraph::new(self.text).block(standard_block(" GitHub Updates ", self.is_selected)).render(area, buf); } }
struct NewsWidget<'a> { text: &'a str, is_selected: bool }
impl<'a> Widget for NewsWidget<'a> { fn render(self, area: Rect, buf: &mut Buffer) { Paragraph::new(self.text).block(standard_block(" Top News ", self.is_selected)).wrap(Wrap { trim: true }).render(area, buf); } }
struct ScheduleWidget<'a> { schedule: &'a [ScheduleItem], is_selected: bool }
impl<'a> Widget for ScheduleWidget<'a> { fn render(self, area: Rect, buf: &mut Buffer) { let items: Vec<ListItem> = self.schedule.iter().map(|i| ListItem::new(format!(" {} | {}", i.time, i.activity))).collect(); List::new(items).block(standard_block(" Schedule ", self.is_selected)).render(area, buf); } }
struct RemindersWidget<'a> { reminders: &'a [ReminderItem], active_idx: usize, is_selected: bool }
impl<'a> Widget for RemindersWidget<'a> { fn render(self, area: Rect, buf: &mut Buffer) { let items: Vec<ListItem> = self.reminders.iter().enumerate().map(|(i, r)| { let sym = if r.is_done { "[x]" } else { "[ ]" }; let style = if i == self.active_idx && self.is_selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() }; ListItem::new(format!(" {} {}", sym, r.task)).style(style) }).collect(); List::new(items).block(standard_block(" Reminders (Space) ", self.is_selected)).render(area, buf); } }

struct FocusedRemindersWidget<'a> { app: &'a App }
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

struct FocusedScheduleWidget<'a> { app: &'a App }
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
}struct FocusedStockOverviewWidget<'a> { app: &'a App }

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
struct FocusedViewWidget<'a> { title: &'a str }
impl<'a> Widget for FocusedViewWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) { Paragraph::new("\n\nDetailed view goes here.\n\nPress 'Esc' or Click anywhere to return.").block(standard_block(self.title, true)).alignment(ratatui::layout::Alignment::Center).render(area, buf); }
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
                        if dist_to_head == 0 { cell.set_fg(Color::White); } else if dist_to_head < 3 { cell.set_fg(Color::LightGreen); } else if dist_to_head > 11 { cell.set_fg(Color::DarkGray); } else { cell.set_fg(Color::Green); }
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
        match app.focused_screen {
            FocusedScreen::Reminders => f.render_widget(FocusedRemindersWidget { app }, inner_area),
            FocusedScreen::Schedule => f.render_widget(FocusedScheduleWidget { app }, inner_area),
            FocusedScreen::StockOverview => f.render_widget(FocusedStockOverviewWidget { app }, inner_area), 
            _ => {
                let title = format!(" Focused: {:?} ", app.focused_screen);
                f.render_widget(FocusedViewWidget { title: &title }, inner_area);
            }
        }
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

    let left_col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(40), Constraint::Percentage(30)])
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
    f.render_widget(WeatherWidget { text: &app.weather_text, is_selected: sel == SelectableWidget::Weather, tick_count: app.tick_count, condition: app.weather_condition }, app.weather_rect);
    
    // FIX 2: Passed `data_list: &app.stock_overview_list` and `tick_count` instead of a non-existent `data` field
    f.render_widget(StockOverviewWidget { 
        data_list: &app.stock_overview_list, 
        tick_count: app.tick_count,
        is_selected: sel == SelectableWidget::StockOverview 
    }, app.stock_overview_rect);
    
    f.render_widget(GithubWidget { text: &app.github_text, is_selected: sel == SelectableWidget::Github }, app.github_rect);
    f.render_widget(NewsWidget { text: &app.news_text, is_selected: sel == SelectableWidget::News }, app.news_rect);
    f.render_widget(ScheduleWidget { schedule: &app.schedule, is_selected: sel == SelectableWidget::Schedule }, app.schedule_rect);
    f.render_widget(RemindersWidget { reminders: &app.reminders, active_idx: app.reminder_index, is_selected: sel == SelectableWidget::Reminders }, app.reminders_rect);

    f.render_widget(MatrixEdgeOverlay { tick: app.tick_count }, full_area);
}