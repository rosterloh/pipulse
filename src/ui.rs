use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Terminal,
};

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
        Paragraph::new(format!("{:<3} {:>3}%", label, pct))
            .style(Style::default().fg(color)),
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

pub fn render<B: Backend>(terminal: &mut Terminal<B>, state: &AppState) {
    terminal
        .draw(|frame| {
            let width = frame.area().width as usize;
            let divider: String = "─".repeat(width);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // [0] header divider
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

            let tail_len = width.saturating_sub(state.hostname.len() + 3);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("─ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(state.hostname.as_str(), Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!(" {}", "─".repeat(tail_len)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])),
                rows[0],
            );

            render_gauge_row(frame, rows[1], "CPU", state.cpu_pct, 50, 80);
            render_gauge_row(frame, rows[3], "Mem", state.mem_pct, 60, 85);
            render_gauge_row(frame, rows[5], "Dsk", state.disk_pct, 70, 90);

            frame.render_widget(
                Paragraph::new(divider.as_str()).style(Style::default().fg(Color::DarkGray)),
                rows[6],
            );

            // Leading spaces leave room for the embedded-icon drawn in main.rs
            frame.render_widget(
                Paragraph::new(format!("   {}", state.ip))
                    .style(Style::default().fg(Color::White)),
                rows[7],
            );

            let temp_str = state
                .temp
                .map_or("   --".into(), |t| format!("   {t:.1}\u{00b0}C"));
            let temp_col = state.temp.map_or(Color::Gray, temp_color);
            frame.render_widget(
                Paragraph::new(temp_str).style(Style::default().fg(temp_col)),
                rows[8],
            );

            frame.render_widget(
                Paragraph::new(format!("   {}", state.uptime))
                    .style(Style::default().fg(Color::Gray)),
                rows[9],
            );

            frame.render_widget(
                Paragraph::new(format!("   {}", state.load))
                    .style(Style::default().fg(Color::Gray)),
                rows[10],
            );
        })
        .unwrap();
}
