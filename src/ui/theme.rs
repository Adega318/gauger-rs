use ratatui::style::{Color, Modifier, Style};

use crate::model::{Health, Runtime, State};

pub fn title() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn header() -> Style {
    Style::default()
        .fg(Color::Gray)
        .add_modifier(Modifier::BOLD)
}

pub fn muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn accent() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn key() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

pub fn ok() -> Style {
    Style::default().fg(Color::Green)
}

pub fn warning() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn error() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
}

pub fn selection() -> Style {
    Style::default()
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

pub fn stale() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn state(state: State) -> Style {
    let color = match state {
        State::Running => Color::Green,
        State::Paused | State::Restarting | State::Created | State::Stopping => Color::Yellow,
        State::Exited | State::Dead | State::Removing => Color::Red,
        State::Empty | State::Unknown => Color::DarkGray,
    };

    Style::default().fg(color)
}

pub fn health(health: Health) -> Style {
    let color = match health {
        Health::Healthy => Color::Green,
        Health::Starting => Color::Yellow,
        Health::Unhealthy => Color::Red,
        Health::Empty | Health::Unknown => Color::DarkGray,
    };

    Style::default().fg(color)
}

pub fn runtime(runtime: Runtime) -> Style {
    let color = match runtime {
        Runtime::Docker => Color::Blue,
        Runtime::Podman => Color::Magenta,
    };

    Style::default().fg(color)
}
