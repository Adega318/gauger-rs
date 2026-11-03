use crossterm::event::KeyEventKind;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Quit,
    Up,
    Down,
    Top,
    Bottom,
    PageUp,
    PageDown,
    Refresh,
    Select,
}

pub fn translate(key: KeyEvent) -> Option<Input> {
    Some(match key.code {
        KeyCode::Char('q') => Input::Quit,
        KeyCode::Char('j') | KeyCode::Down => Input::Down,
        KeyCode::Char('k') | KeyCode::Up => Input::Up,
        KeyCode::Char('g') | KeyCode::Home => Input::Top,
        KeyCode::Char('G') | KeyCode::End => Input::Bottom,
        KeyCode::PageDown => Input::PageDown,
        KeyCode::PageUp => Input::PageUp,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Input::PageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Input::PageUp,
        KeyCode::Char('r') => Input::Refresh,
        KeyCode::Enter => Input::Select,
        _ => return None,
    })
}

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return;
        }

        if let Some(input) = translate(key) {
            self.handle_input(input);
        }
    }

    fn handle_input(&mut self, input: Input) {
        match input {
            Input::Refresh => self.request_refresh(),
            Input::Down => self.move_selection(1),
            Input::Up => self.move_selection(-1),
            Input::Top => self.selected = 0,
            Input::Bottom => self.selected = self.visible.len().saturating_sub(1),
            Input::PageDown => self.move_selection(10),
            Input::PageUp => self.move_selection(-10),

            Input::Quit if self.show_detail => self.show_detail = false,
            Input::Quit => self.should_quit = true,

            _ if self.show_detail => {}

            Input::Select => self.show_detail = true,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            self.selected = 0;
            return;
        }

        let last = self.visible.len() - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last as isize) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn maps_navigation() {
        assert_eq!(translate(key(KeyCode::Char('j'))), Some(Input::Down));
        assert_eq!(translate(key(KeyCode::Down)), Some(Input::Down));
        assert_eq!(translate(key(KeyCode::Char('k'))), Some(Input::Up));
        assert_eq!(translate(key(KeyCode::Char('g'))), Some(Input::Top));
        assert_eq!(translate(key(KeyCode::Char('G'))), Some(Input::Bottom));
        assert_eq!(
            translate(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            Some(Input::PageDown)
        );
    }

    #[test]
    fn unhandled_keys_are_ignored() {
        assert_eq!(translate(key(KeyCode::F(5))), None);
        assert_eq!(
            translate(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            None
        );
    }
}
