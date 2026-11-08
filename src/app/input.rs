use crossterm::event::KeyEventKind;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Top,
    Bottom,
    PageUp,
    PageDown,
    Refresh,
    Select,
    Search,
    Escape,
}

pub fn translate(key: KeyEvent) -> Option<Input> {
    Some(match key.code {
        KeyCode::Char('/') => Input::Search,
        KeyCode::Esc => Input::Escape,
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

        if self.is_searching() {
            self.handle_search_key(key);
            return;
        }

        if let Some(input) = translate(key) {
            self.handle_input(input);
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.confirm_search(),
            KeyCode::Esc => self.clear_search(),
            KeyCode::Backspace => self.pop_search_char(),
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.push_search_char(character);
            }
            _ => {}
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

            Input::Escape if self.show_detail => self.show_detail = false,
            Input::Escape if self.is_filtered() => self.clear_search(),
            Input::Escape => self.should_quit = true,

            _ if self.show_detail => {}

            Input::Search => self.begin_search(),
            Input::Select if self.visible_len() > 0 => self.show_detail = true,
            Input::Select => {}
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

    #[test]
    fn maps_search_and_escape_keys() {
        assert_eq!(translate(key(KeyCode::Char('/'))), Some(Input::Search));
        assert_eq!(translate(key(KeyCode::Esc)), Some(Input::Escape));
    }

    #[test]
    fn search_mode_takes_priority_over_keybindings() {
        let mut app = App::new(vec![crate::model::Runtime::Docker]);

        app.handle_key(key(KeyCode::Char('/')));
        assert!(app.is_searching());

        app.handle_key(key(KeyCode::Char('q')));
        app.handle_key(key(KeyCode::Char('j')));
        assert!(!app.should_quit);
        assert_eq!(app.search_query(), "qj");

        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(app.search_query(), "q");

        app.handle_key(key(KeyCode::Enter));
        assert!(!app.is_searching());
        assert_eq!(app.search_query(), "q");

        app.handle_key(key(KeyCode::Esc));
        assert!(!app.should_quit);
        assert_eq!(app.search_query(), "");
    }

    #[test]
    fn esc_quits_when_idle() {
        let mut app = App::new(vec![crate::model::Runtime::Docker]);

        app.handle_key(key(KeyCode::Esc));

        assert!(app.should_quit);
    }

    #[test]
    fn esc_clears_search_before_quitting() {
        let mut app = App::new(vec![crate::model::Runtime::Docker]);
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Enter));

        app.handle_key(key(KeyCode::Esc));

        assert!(!app.should_quit);
        assert!(!app.is_filtered());

        app.handle_key(key(KeyCode::Esc));

        assert!(app.should_quit);
    }

    #[test]
    fn esc_closes_detail_before_quitting() {
        let mut app = App::new(vec![crate::model::Runtime::Docker]);
        app.show_detail = true;

        app.handle_key(key(KeyCode::Esc));

        assert!(!app.should_quit);
        assert!(!app.show_detail);

        app.handle_key(key(KeyCode::Esc));

        assert!(app.should_quit);
    }

    #[test]
    fn q_no_longer_quits() {
        let mut app = App::new(vec![crate::model::Runtime::Docker]);

        app.handle_key(key(KeyCode::Char('q')));

        assert!(!app.should_quit);
    }

    #[test]
    fn search_mode_ignores_control_character_input() {
        let mut app = App::new(vec![crate::model::Runtime::Docker]);
        app.handle_key(key(KeyCode::Char('/')));

        app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));

        assert_eq!(app.search_query(), "");
    }

    #[test]
    fn control_c_still_quits_while_searching() {
        let mut app = App::new(vec![crate::model::Runtime::Docker]);
        app.handle_key(key(KeyCode::Char('/')));

        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

        assert!(app.should_quit);
    }

    #[test]
    fn select_is_ignored_when_search_has_no_results() {
        let mut app = App::new(vec![crate::model::Runtime::Docker]);

        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.visible_len(), 0);
        assert!(!app.show_detail);
    }
}
