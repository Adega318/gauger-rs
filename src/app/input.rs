use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Quit,
    Up,
    Down,
    Top,
    Bottom,
    PageUp,
    PageDown,
    StartFilter,
    ApplyFilter,
    CancelFilter,
    ClearFilter,
    Char(char),
    Backspace,
    CycleSort,
    ReverseSort,
    Refresh,
    OpenDetails,
    Help,
    CloseModal,
}

pub fn translate(key: KeyEvent, mode: &Mode) -> Option<Input> {
    match mode {
        Mode::Normal => translate_normal(key),
        Mode::Filter => translate_filter(key),
        Mode::Details(_) | Mode::Help => translate_modal(key),
    }
}

fn translate_normal(key: KeyEvent) -> Option<Input> {
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
        KeyCode::Char('/') => Input::StartFilter,
        KeyCode::Char('c') | KeyCode::Esc => Input::ClearFilter,
        KeyCode::Char('s') => Input::CycleSort,
        KeyCode::Char('S') => Input::ReverseSort,
        KeyCode::Char('r') => Input::Refresh,
        KeyCode::Enter => Input::OpenDetails,
        KeyCode::Char('?') | KeyCode::F(1) => Input::Help,
        _ => return None,
    })
}

fn translate_filter(key: KeyEvent) -> Option<Input> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);

    Some(match key.code {
        KeyCode::Char('u') if control => Input::ClearFilter,
        KeyCode::Char(character) if !control => Input::Char(character),
        KeyCode::Backspace => Input::Backspace,
        KeyCode::Enter => Input::ApplyFilter,
        KeyCode::Esc => Input::CancelFilter,
        _ => return None,
    })
}

fn translate_modal(key: KeyEvent) -> Option<Input> {
    Some(match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => Input::CloseModal,
        KeyCode::Char('?') | KeyCode::F(1) => Input::CloseModal,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn maps_normal_mode_navigation() {
        assert_eq!(
            translate(key(KeyCode::Char('j')), &Mode::Normal),
            Some(Input::Down)
        );
        assert_eq!(
            translate(key(KeyCode::Down), &Mode::Normal),
            Some(Input::Down)
        );
        assert_eq!(
            translate(key(KeyCode::Char('k')), &Mode::Normal),
            Some(Input::Up)
        );
        assert_eq!(
            translate(key(KeyCode::Char('g')), &Mode::Normal),
            Some(Input::Top)
        );
        assert_eq!(
            translate(key(KeyCode::Char('G')), &Mode::Normal),
            Some(Input::Bottom)
        );
        assert_eq!(
            translate(
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
                &Mode::Normal
            ),
            Some(Input::PageDown)
        );
    }

    #[test]
    fn maps_normal_mode_actions() {
        assert_eq!(
            translate(key(KeyCode::Char('/')), &Mode::Normal),
            Some(Input::StartFilter)
        );
        assert_eq!(
            translate(key(KeyCode::Char('s')), &Mode::Normal),
            Some(Input::CycleSort)
        );
        assert_eq!(
            translate(key(KeyCode::Char('S')), &Mode::Normal),
            Some(Input::ReverseSort)
        );
        assert_eq!(
            translate(key(KeyCode::Enter), &Mode::Normal),
            Some(Input::OpenDetails)
        );
        assert_eq!(
            translate(key(KeyCode::Char('?')), &Mode::Normal),
            Some(Input::Help)
        );
        assert_eq!(
            translate(key(KeyCode::Char('r')), &Mode::Normal),
            Some(Input::Refresh)
        );
        assert_eq!(
            translate(key(KeyCode::Char('q')), &Mode::Normal),
            Some(Input::Quit)
        );
    }

    #[test]
    fn filter_mode_edits_text() {
        assert_eq!(
            translate(key(KeyCode::Char('a')), &Mode::Filter),
            Some(Input::Char('a'))
        );
        assert_eq!(
            translate(key(KeyCode::Backspace), &Mode::Filter),
            Some(Input::Backspace)
        );
        assert_eq!(
            translate(key(KeyCode::Enter), &Mode::Filter),
            Some(Input::ApplyFilter)
        );
        assert_eq!(
            translate(key(KeyCode::Esc), &Mode::Filter),
            Some(Input::CancelFilter)
        );
        assert_eq!(
            translate(
                KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
                &Mode::Filter
            ),
            Some(Input::ClearFilter)
        );
    }

    #[test]
    fn modal_mode_closes_on_escape_enter_or_q() {
        let mode = Mode::Help;
        for code in [KeyCode::Esc, KeyCode::Enter, KeyCode::Char('q')] {
            assert_eq!(translate(key(code), &mode), Some(Input::CloseModal));
        }
    }

    #[test]
    fn unhandled_keys_are_ignored() {
        assert_eq!(translate(key(KeyCode::F(5)), &Mode::Normal), None);
        assert_eq!(
            translate(
                KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
                &Mode::Filter
            ),
            None
        );
    }
}
