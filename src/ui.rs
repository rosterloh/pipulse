use crate::metrics::Snapshot;
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph, Sparkline},
};
use std::collections::VecDeque;

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

pub struct AppState<'a> {
    pub snap: Snapshot,
    pub page: Page,
    pub rx_history: &'a VecDeque<u64>,
    pub tx_history: &'a VecDeque<u64>,
}

const GREEN: Color = Color::Rgb(100, 185, 100);
const AMBER: Color = Color::Rgb(210, 170, 70);
const RED: Color = Color::Rgb(210, 90, 80);
/// Unfilled part of a gauge: dark, so only the filled part stands out.
const TRACK: Color = Color::Rgb(40, 40, 40);
/// Labels and secondary values.
const DIM: Color = Color::Rgb(110, 110, 110);
/// Horizontal rules.
const RULE: Color = Color::Rgb(60, 60, 60);

/// First text row of the Overview info rows (IP, temp, uptime, load), each two
/// rows tall. main.rs draws the icons there.
pub const OVERVIEW_ICON_ROW: i32 = 8;

/// Leading spaces on the Overview info rows, leaving room for the icons that
/// main.rs draws over them.
const ICON_PAD: &str = "    ";

fn traffic_color(pct: u8, warn: u8, crit: u8) -> Color {
    if pct >= crit {
        RED
    } else if pct >= warn {
        AMBER
    } else {
        GREEN
    }
}

fn temp_color(temp: f32) -> Color {
    if temp >= 70.0 {
        RED
    } else if temp >= 55.0 {
        AMBER
    } else {
        GREEN
    }
}

/// Load is fine below the core count, busy above it, overloaded above twice.
fn load_color(load: f64, cores: usize) -> Color {
    let cores = cores.max(1) as f64;
    if load >= 2.0 * cores {
        RED
    } else if load >= cores {
        AMBER
    } else {
        GREEN
    }
}

fn bold(color: Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn dim(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(DIM))
}

fn render_gauge_row(frame: &mut Frame<'_>, area: Rect, label: &str, pct: u8, warn: u8, crit: u8) {
    let color = traffic_color(pct, warn, crit);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            dim(format!("{label:<3} ")),
            Span::styled(format!("{pct:>3}%"), bold(color)),
        ])),
        cols[0],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(color).bg(TRACK))
            .ratio(pct as f64 / 100.0)
            .label(""),
        cols[1],
    );
}

/// Header line: bold title on the left, then the clock and one dot per page.
fn render_header(frame: &mut Frame<'_>, area: Rect, title: &str, time: &str, page: Page) {
    let dots: String = (1..=Page::COUNT)
        .map(|i| if i == page.index() { '●' } else { '○' })
        .collect();
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(time.chars().count() as u16 + 1),
            Constraint::Length(Page::COUNT as u16),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(Span::styled(title.to_string(), bold(Color::Cyan))),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            time.to_string(),
            Style::default().fg(Color::White),
        )),
        cols[1],
    );
    frame.render_widget(Paragraph::new(dim(dots)), cols[2]);
}

fn render_rule(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(RULE)),
        area,
    );
}

fn fmt_rate(bps: u64) -> String {
    format!("{}/s", fmt_bytes(bps))
}

fn fmt_bytes(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} KB", b / 1e3)
    } else {
        format!("{bytes} B")
    }
}

fn render_overview(frame: &mut Frame<'_>, state: &AppState<'_>) {
    let m = &state.snap;
    // Rows 8-11 (`OVERVIEW_ICON_ROW`) are two text rows tall to fit the icons.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0] header
            Constraint::Length(1), // [1] gap
            Constraint::Length(1), // [2] CPU
            Constraint::Length(1), // [3] gap
            Constraint::Length(1), // [4] Mem
            Constraint::Length(1), // [5] gap
            Constraint::Length(1), // [6] Disk
            Constraint::Length(1), // [7] rule
            Constraint::Length(2), // [8] IP
            Constraint::Length(2), // [9] Temp
            Constraint::Length(2), // [10] Uptime
            Constraint::Length(2), // [11] Load
            Constraint::Min(0),
        ])
        .split(frame.area());

    render_header(frame, rows[0], &m.hostname, &m.time, state.page);

    render_gauge_row(frame, rows[2], "CPU", m.cpu_pct, 50, 80);
    render_gauge_row(frame, rows[4], "Mem", m.mem_pct, 60, 85);
    render_gauge_row(frame, rows[6], "Dsk", m.disk_pct, 70, 90);
    render_rule(frame, rows[7]);

    let label = |l: &str| dim(format!("{ICON_PAD}{l:<5}"));

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(ICON_PAD),
            Span::styled(m.ip.clone(), bold(Color::White)),
        ])),
        rows[8],
    );

    let temp = match m.temp {
        Some(t) => Span::styled(format!("{t:.1}\u{00b0}C"), bold(temp_color(t))),
        None => dim("--"),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![label("temp"), temp])),
        rows[9],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            label("up"),
            Span::styled(m.uptime.clone(), Style::default().fg(Color::White)),
        ])),
        rows[10],
    );

    let [one, five, fifteen] = m.load;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            label("load"),
            Span::styled(format!("{one:.2}"), bold(load_color(one, m.cores))),
            dim(format!(" {five:.2} {fifteen:.2}")),
        ])),
        rows[11],
    );
}

fn render_network(frame: &mut Frame<'_>, state: &AppState<'_>) {
    let m = &state.snap;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0] header
            Constraint::Length(1), // [1] gap
            Constraint::Length(1), // [2] iface + IP
            Constraint::Length(1), // [3] rule
            Constraint::Length(1), // [4] Rx label
            Constraint::Length(5), // [5] Rx sparkline
            Constraint::Length(1), // [6] Tx label
            Constraint::Length(5), // [7] Tx sparkline
            Constraint::Length(1), // [8] rule
            Constraint::Length(1), // [9] totals
            Constraint::Min(0),
        ])
        .split(frame.area());

    render_header(frame, rows[0], "Network", &m.time, state.page);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            dim(format!("{} ", m.iface)),
            Span::styled(m.ip.clone(), Style::default().fg(Color::White)),
        ])),
        rows[2],
    );
    render_rule(frame, rows[3]);

    for (label_row, spark_row, name, history, color) in [
        (rows[4], rows[5], "Rx \u{2193}", state.rx_history, GREEN),
        (
            rows[6],
            rows[7],
            "Tx \u{2191}",
            state.tx_history,
            Color::Cyan,
        ),
    ] {
        let current = history.back().copied().unwrap_or(0);
        let peak = history.iter().copied().max().unwrap_or(0);
        let peak_text = format!("peak {}", fmt_rate(peak));
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(peak_text.chars().count() as u16),
            ])
            .split(label_row);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{name} "), Style::default().fg(color)),
                Span::styled(fmt_rate(current), bold(color)),
            ])),
            cols[0],
        );
        frame.render_widget(Paragraph::new(dim(peak_text)), cols[1]);
        frame.render_widget(
            Sparkline::default()
                .data(history.iter().copied())
                .style(Style::default().fg(color)),
            spark_row,
        );
    }

    render_rule(frame, rows[8]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            dim("total "),
            Span::styled(
                format!(
                    "\u{2193} {}  \u{2191} {}",
                    fmt_bytes(m.rx_total),
                    fmt_bytes(m.tx_total)
                ),
                Style::default().fg(Color::White),
            ),
        ])),
        rows[9],
    );
}

pub fn render<B: Backend>(
    terminal: &mut Terminal<B>,
    state: &AppState<'_>,
) -> Result<(), B::Error> {
    terminal.draw(|frame| match state.page {
        Page::Overview => render_overview(frame, state),
        Page::Network => render_network(frame, state),
    })?;
    Ok(())
}
