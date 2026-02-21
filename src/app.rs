use crate::{
    schedule::{ReminderItem, ScheduleItem},
    stocks::{DetailedTickerData, StockOverviewData, fetch_detailed_ticker},
    weather::WeatherCondition,
    shared::load_json,
};
use ratatui::layout::Rect;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use std::sync::mpsc;

pub enum AppEvent {
    UpdateStock(String),
    UpdateWeather(String, WeatherCondition),
    UpdateTime(String),
    UpdateGithub(String),
    UpdateNews(String),
    UpdateStockOverview(StockOverviewData),
    UpdateDetailedTicker(DetailedTickerData),
}

pub enum Action {
    Tick,
    Input(Event),
    Backend(AppEvent),
    Quit,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FocusedScreen { Dashboard, Time, Stock, Weather, StockOverview, Github, News, Schedule, Reminders }

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SelectableWidget { Stock, Weather, StockOverview, Github, News, Schedule, Reminders }

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum InputMode { Normal, AddingReminder, AddingScheduleTime, AddingScheduleActivity, SearchingTicker }

pub struct App {
    pub focused_screen: FocusedScreen,
    pub selected_widget: SelectableWidget,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub temp_schedule_time: String,
    pub tick_count: u64,
    pub stock_text: String,
    pub weather_text: String,
    pub weather_condition: WeatherCondition,
    pub time_text: String,
    pub github_text: String,
    pub news_text: String,
    pub stock_overview_list: Vec<StockOverviewData>,
    pub detailed_ticker_data: DetailedTickerData,
    pub schedule: Vec<ScheduleItem>,
    pub schedule_index: usize,
    pub reminders: Vec<ReminderItem>,
    pub reminder_index: usize,
    pub time_rect: Rect, pub stock_rect: Rect, pub weather_rect: Rect,
    pub stock_overview_rect: Rect, pub github_rect: Rect, pub news_rect: Rect,
    pub schedule_rect: Rect, pub reminders_rect: Rect,
    pub tx: Option<mpsc::Sender<AppEvent>>,
}

impl App {
    pub fn new(tx: mpsc::Sender<AppEvent>) -> Self {
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
            stock_overview_list: Vec::new(),
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

    pub fn save_reminders(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.reminders) {
            let _ = std::fs::write("reminders.json", json);
        }
    }

    pub fn save_schedule(&mut self) {
        self.schedule.sort_by(|a, b| a.time.cmp(&b.time));
        if let Ok(json) = serde_json::to_string_pretty(&self.schedule) {
            let _ = std::fs::write("schedule.json", json);
        }
    }

    pub fn toggle_reminder(&mut self) {
        if !self.reminders.is_empty() {
            self.reminders[self.reminder_index].is_done = !self.reminders[self.reminder_index].is_done;
            self.save_reminders();
        }
    }

    pub fn handle_click(&mut self, x: u16, y: u16) {
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

    pub fn move_selection(&mut self, key: KeyCode) {
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

    // Returns true if app should quit
    pub fn handle_action(&mut self, action: Action) -> bool {
        match action {
            Action::Tick => {
                self.tick_count = self.tick_count.wrapping_add(1);
            }
            Action::Backend(event) => match event {
                AppEvent::UpdateStock(s) => self.stock_text = s,
                AppEvent::UpdateWeather(s, c) => {
                    self.weather_text = s;
                    self.weather_condition = c;
                },
                AppEvent::UpdateTime(s) => self.time_text = s,
                AppEvent::UpdateGithub(s) => self.github_text = s,
                AppEvent::UpdateNews(s) => self.news_text = s,
                AppEvent::UpdateStockOverview(d) => {
                    if let Some(pos) = self.stock_overview_list.iter().position(|s| s.symbol == d.symbol) {
                        self.stock_overview_list[pos] = d;
                    } else {
                        self.stock_overview_list.push(d);
                    }
                },
                AppEvent::UpdateDetailedTicker(d) => {
                    self.detailed_ticker_data = d;
                }
            },
            Action::Input(event) => match event {
                Event::Key(key) => {
                    if key.code == KeyCode::Esc {
                        self.focused_screen = FocusedScreen::Dashboard;
                        self.input_mode = InputMode::Normal;
                        self.input_buffer.clear();
                        return false;
                    }

                    if self.input_mode != InputMode::Normal {
                        match key.code {
                            KeyCode::Enter => {
                                match self.input_mode {
                                    InputMode::AddingReminder => {
                                        if !self.input_buffer.trim().is_empty() {
                                            self.reminders.push(ReminderItem { task: self.input_buffer.clone(), is_done: false });
                                            self.save_reminders();
                                        }
                                        self.input_mode = InputMode::Normal;
                                    }
                                    InputMode::AddingScheduleTime => {
                                        if !self.input_buffer.trim().is_empty() {
                                            self.temp_schedule_time = self.input_buffer.clone();
                                            self.input_mode = InputMode::AddingScheduleActivity;
                                        } else { self.input_mode = InputMode::Normal; }
                                    }
                                    InputMode::AddingScheduleActivity => {
                                        if !self.input_buffer.trim().is_empty() {
                                            self.schedule.push(ScheduleItem {
                                                time: self.temp_schedule_time.clone(),
                                                activity: self.input_buffer.clone()
                                            });
                                            self.save_schedule();
                                        }
                                        self.input_mode = InputMode::Normal;
                                    }
                                    InputMode::SearchingTicker => {
                                        if !self.input_buffer.trim().is_empty() {
                                            if let Some(tx) = &self.tx {
                                                self.detailed_ticker_data.is_loading = true;
                                                let symbol = self.input_buffer.trim().to_string();
                                                let tx_clone = tx.clone();
                                                tokio::spawn(async move {
                                                    fetch_detailed_ticker(symbol, tx_clone).await;
                                                });
                                            }
                                        }
                                        self.input_mode = InputMode::Normal;
                                    }
                                    _ => {}
                                }
                                self.input_buffer.clear();
                            }
                            KeyCode::Backspace => { self.input_buffer.pop(); }
                            KeyCode::Char(c) => { self.input_buffer.push(c); }
                            _ => {}
                        }
                        return false;
                    }

                    if key.code == KeyCode::Char('q') { return true; }

                    match self.focused_screen {
                        FocusedScreen::Dashboard => {
                            if key.code == KeyCode::Char(' ') && self.selected_widget == SelectableWidget::Reminders {
                                self.toggle_reminder();
                            }
                            match key.code {
                                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => self.move_selection(key.code),
                                KeyCode::Enter => {
                                    self.focused_screen = match self.selected_widget {
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
                                self.input_mode = InputMode::SearchingTicker;
                                self.input_buffer.clear();
                            }
                        }
                        FocusedScreen::Reminders => {
                            match key.code {
                                KeyCode::Up => { if self.reminder_index > 0 { self.reminder_index -= 1; } }
                                KeyCode::Down => { if self.reminder_index < self.reminders.len().saturating_sub(1) { self.reminder_index += 1; } }
                                KeyCode::Char(' ') | KeyCode::Enter => { self.toggle_reminder(); }
                                KeyCode::Char('a') => { self.input_mode = InputMode::AddingReminder; self.input_buffer.clear(); }
                                KeyCode::Char('d') | KeyCode::Backspace => {
                                    if !self.reminders.is_empty() {
                                        self.reminders.remove(self.reminder_index);
                                        if self.reminder_index >= self.reminders.len() { self.reminder_index = self.reminders.len().saturating_sub(1); }
                                        self.save_reminders();
                                    }
                                }
                                _ => {}
                            }
                        }
                        FocusedScreen::Schedule => {
                            match key.code {
                                KeyCode::Up => { if self.schedule_index > 0 { self.schedule_index -= 1; } }
                                KeyCode::Down => { if self.schedule_index < self.schedule.len().saturating_sub(1) { self.schedule_index += 1; } }
                                KeyCode::Char('a') => { self.input_mode = InputMode::AddingScheduleTime; self.input_buffer.clear(); }
                                KeyCode::Char('d') | KeyCode::Backspace => {
                                    if !self.schedule.is_empty() {
                                        self.schedule.remove(self.schedule_index);
                                        if self.schedule_index >= self.schedule.len() { self.schedule_index = self.schedule.len().saturating_sub(1); }
                                        self.save_schedule();
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
                        self.handle_click(mouse.column, mouse.row);
                    }
                }
                _ => {}
            },
            Action::Quit => return true,
        }
        false
    }
}