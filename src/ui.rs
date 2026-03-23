use std::collections::VecDeque;
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph, Sparkline},
};

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Page {
    #[default]
    Overview,
    Network,
}

impl Page {
    pub const COUNT: usize = 2;

    pub fn next(self) -> Self {
        match self {
            Page::Overview => Page::Network,
            Page::Network => Page::Overview,
        }
    }

    pub fn prev(self) -> Self {
        // Two pages: prev wraps the same way as next.
        match self {
            Page::Overview => Page::Network,
            Page::Network => Page::Overview,
        }
    }

    fn index(self) -> usize {
        match self {
            Page::Overview => 1,
            Page::Network => 2,
        }
    }
}

pub struct AppState {
    pub ip: String,
    pub iface: String,
    pub hostname: String,
    pub cpu_pct: u8,
    pub mem_pct: u8,
    pub disk_pct: u8,
    pub temp: Option<f32>,
    pub uptime: String,
    pub load: String,
    pub page: Page,
    pub rx_history: VecDeque<u64>,
    pub tx_history: VecDeque<u64>,
}

fn traffic_color(pct: u8, warn: u8, crit: u8) -> Color {
    if pct >= crit {
        Color::Rgb(210, 90, 80)
    } else if pct >= warn {
        Color::Rgb(210, 170, 70)
    } else {
        Color::Rgb(100, 185, 100)
    }
}

fn temp_color(temp: f32) -> Color {
    if temp >= 70.0 {
        Color::Rgb(210, 90, 80)
    } else if temp >= 55.0 {
        Color::Rgb(210, 170, 70)
    } else {
        Color::Rgb(100, 185, 100)
    }
}

fn render_gauge_row(frame: &mut Frame<'_>, area: Rect, label: &str, pct: u8, warn: u8, crit: u8) {
    let color = traffic_color(pct, warn, crit);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(area);
    frame.render_widget(
        Paragraph::new(format!("{:<3} {:>3}%", label, pct)).style(Style::default().fg(color)),
        cols[0],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
            .ratio(pct as f64 / 100.0)
            .label(""),
        cols[1],
    );
}

/// Renders a header line: "─ <title> ─...─ N/M"
fn render_page_header(frame: &mut Frame<'_>, area: Rect, title: &str, page: Page) {
    let width = area.width as usize;
    let indicator = format!("{}/{}", page.index(), Page::COUNT);
    // Layout: "─ " (2) + title + " ─...─ " (1 + dashes + 2) + indicator
    // Fixed characters (excluding variable dashes): 2 + title.len() + 3 + indicator.len()
    let fixed = 2 + title.len() + 3 + indicator.len();
    let extra_dashes = width.saturating_sub(fixed);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("─ ", Style::default().fg(Color::DarkGray)),
            Span::styled(title.to_string(), Style::default().fg(Color::Cyan)),
            Span::styled(
                format!(" {}─ ", "─".repeat(extra_dashes)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(indicator, Style::default().fg(Color::White)),
        ])),
        area,
    );
}

fn fmt_rate(bps: u64) -> String {
    if bps >= 1_000_000 {
        format!("{:.1} MB/s", bps as f64 / 1_000_000.0)
    } else if bps >= 1_000 {
        format!("{:.0} KB/s", bps as f64 / 1_000.0)
    } else {
        format!("{} B/s", bps)
    }
}

fn render_overview(frame: &mut Frame<'_>, state: &AppState) {
    let divider: String = "─".repeat(frame.area().width as usize);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0] header
            Constraint::Length(1), // [1] CPU
            Constraint::Length(1), // [2] spacer
            Constraint::Length(1), // [3] Mem
            Constraint::Length(1), // [4] spacer
            Constraint::Length(1), // [5] Disk
            Constraint::Length(1), // [6] divider
            Constraint::Length(2), // [7] IP
            Constraint::Length(2), // [8] Temp
            Constraint::Length(2), // [9] Uptime
            Constraint::Length(2), // [10] Load
            Constraint::Min(0),
        ])
        .split(frame.area());

    render_page_header(frame, rows[0], &state.hostname, state.page);

    render_gauge_row(frame, rows[1], "CPU", state.cpu_pct, 50, 80);
    render_gauge_row(frame, rows[3], "Mem", state.mem_pct, 60, 85);
    render_gauge_row(frame, rows[5], "Dsk", state.disk_pct, 70, 90);

    frame.render_widget(
        Paragraph::new(divider.as_str()).style(Style::default().fg(Color::DarkGray)),
        rows[6],
    );

    // Leading spaces leave room for the embedded-icon drawn in main.rs
    frame.render_widget(
        Paragraph::new(format!("    {}", state.ip))
            .style(Style::default().fg(Color::White)),
        rows[7],
    );

    let temp_str = state
        .temp
        .map_or("    --".into(), |t| format!("    {t:.1}\u{00b0}C"));
    let temp_col = state.temp.map_or(Color::Gray, temp_color);
    frame.render_widget(
        Paragraph::new(temp_str).style(Style::default().fg(temp_col)),
        rows[8],
    );

    frame.render_widget(
        Paragraph::new(format!("    {}", state.uptime))
            .style(Style::default().fg(Color::Gray)),
        rows[9],
    );

    frame.render_widget(
        Paragraph::new(format!("    {}", state.load))
            .style(Style::default().fg(Color::Gray)),
        rows[10],
    );
}

fn render_network(frame: &mut Frame<'_>, state: &AppState) {
    let divider: String = "─".repeat(frame.area().width as usize);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0] header (iface + IP + page indicator)
            Constraint::Length(1), // [1] divider
            Constraint::Length(1), // [2] Rx label + rate
            Constraint::Length(3), // [3] Rx sparkline
            Constraint::Length(1), // [4] Tx label + rate
            Constraint::Length(3), // [5] Tx sparkline
            Constraint::Min(0),
        ])
        .split(frame.area());

    let iface_ip = format!("{} {}", state.iface, state.ip);
    render_page_header(frame, rows[0], &iface_ip, state.page);

    frame.render_widget(
        Paragraph::new(divider.as_str()).style(Style::default().fg(Color::DarkGray)),
        rows[1],
    );

    let rx_data: Vec<u64> = state.rx_history.iter().copied().collect();
    let tx_data: Vec<u64> = state.tx_history.iter().copied().collect();
    let current_rx = rx_data.last().copied().unwrap_or(0);
    let current_tx = tx_data.last().copied().unwrap_or(0);

    frame.render_widget(
        Paragraph::new(format!("Rx  {}", fmt_rate(current_rx)))
            .style(Style::default().fg(Color::Green)),
        rows[2],
    );
    frame.render_widget(
        Sparkline::default()
            .data(&rx_data)
            .style(Style::default().fg(Color::Green)),
        rows[3],
    );

    frame.render_widget(
        Paragraph::new(format!("Tx  {}", fmt_rate(current_tx)))
            .style(Style::default().fg(Color::Cyan)),
        rows[4],
    );
    frame.render_widget(
        Sparkline::default()
            .data(&tx_data)
            .style(Style::default().fg(Color::Cyan)),
        rows[5],
    );
}

pub fn render<B: Backend>(terminal: &mut Terminal<B>, state: &AppState) {
    terminal
        .draw(|frame| match state.page {
            Page::Overview => render_overview(frame, state),
            Page::Network => render_network(frame, state),
        })
        .unwrap();
}
