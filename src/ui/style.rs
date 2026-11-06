use ratatui::style::{Color, Modifier, Style};

use crate::model::{Health, Runtime, State};

// Base
pub fn normal() -> Style {
    Style::default().fg(Color::White)
}

pub fn muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn bold() -> Style {
    normal().add_modifier(Modifier::BOLD)
}

// Branding
pub fn title() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn highlight() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

// Table
pub fn header() -> Style {
    Style::default()
        .fg(Color::Gray)
        .add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default().bg(Color::Rgb(38, 43, 58))
}

pub fn project() -> Style {
    Style::default()
        .fg(Color::Rgb(203, 213, 225))
        .bg(Color::Rgb(51, 65, 85))
        .add_modifier(Modifier::BOLD)
}

// Status
pub fn success() -> Style {
    Style::default().fg(Color::Green)
}

pub fn warning() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn danger() -> Style {
    Style::default().fg(Color::Red)
}

pub fn state(state: State) -> Style {
    match state {
        State::Running => success(),
        State::Exited | State::Dead => danger(),
        State::Paused | State::Stopping | State::Removing => muted(),
        State::Created | State::Restarting => warning(),
        State::Empty => muted(),
    }
}

pub fn usage_percent(percent: f32) -> Style {
    if percent >= 80.0 {
        danger()
    } else if percent >= 50.0 {
        warning()
    } else {
        normal()
    }
}

pub fn health(health: Health) -> Style {
    match health {
        Health::Healthy => success(),
        Health::Unhealthy => danger(),
        Health::Starting => warning(),
        Health::Empty => muted(),
    }
}

pub fn runtime(runtime: Runtime) -> Style {
    match runtime {
        Runtime::Docker => Style::default().fg(Color::Blue),
        Runtime::Podman => Style::default().fg(Color::Magenta),
    }
}

// Layout
pub fn border() -> Style {
    muted()
}

// Footer
pub fn shortcut() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}
