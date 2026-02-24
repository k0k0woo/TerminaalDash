use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use crate::{
    app::{App, FocusedScreen, SelectableWidget},
    shared::{TimeWidget, FocusedViewWidget},
    overlay::{ThunderstormOverlay},
    weather::WeatherWidget,
    stocks::{StockWidget, StockOverviewWidget, FocusedStockOverviewWidget},
    github::GithubWidget,
    news::NewsWidget,
    schedule::{ScheduleWidget, RemindersWidget, FocusedScheduleWidget, FocusedRemindersWidget},
};

pub fn ui(f: &mut Frame, app: &mut App) {
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
        f.render_widget(ThunderstormOverlay { tick: app.tick_count }, full_area);
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
    
    f.render_widget(StockOverviewWidget { 
        data_list: &app.stock_overview_list, 
        tick_count: app.tick_count,
        is_selected: sel == SelectableWidget::StockOverview 
    }, app.stock_overview_rect);
    
    f.render_widget(GithubWidget { text: &app.github_text, is_selected: sel == SelectableWidget::Github }, app.github_rect);
    f.render_widget(NewsWidget { text: &app.news_text, is_selected: sel == SelectableWidget::News,tick_count: app.tick_count}, app.news_rect);
    f.render_widget(ScheduleWidget { schedule: &app.schedule, is_selected: sel == SelectableWidget::Schedule }, app.schedule_rect);
    f.render_widget(RemindersWidget { reminders: &app.reminders, active_idx: app.reminder_index, is_selected: sel == SelectableWidget::Reminders }, app.reminders_rect);

    f.render_widget(ThunderstormOverlay { tick: app.tick_count }, full_area);
}