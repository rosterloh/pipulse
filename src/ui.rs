use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

pub struct AppState {
    pub ip: String,
    pub load: String,
}

pub fn render<B: Backend>(terminal: &mut Terminal<B>, state: &AppState) {
    terminal
        .draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(frame.area());

            let title = Paragraph::new("PiPulse")
                .block(Block::default().borders(Borders::BOTTOM))
                .style(Style::default().fg(Color::Cyan));
            frame.render_widget(title, chunks[0]);

            let metrics = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Length(3)])
                .split(chunks[1]);

            let ip_widget = Paragraph::new(state.ip.as_str())
                .block(Block::default().title("IP").borders(Borders::ALL))
                .style(Style::default().fg(Color::White));
            frame.render_widget(ip_widget, metrics[0]);

            let load_widget = Paragraph::new(state.load.as_str())
                .block(Block::default().title("Load").borders(Borders::ALL))
                .style(Style::default().fg(Color::Yellow));
            frame.render_widget(load_widget, metrics[1]);
        })
        .unwrap();
}
