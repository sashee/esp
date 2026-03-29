use crossterm::event::KeyEvent;
use ratatui::layout::Rect;

#[derive(Clone, Debug)]
pub enum FormResult {
    Continue,
    Save { items: Vec<serde_json::Value> },
    Cancel,
}

pub trait FormController {
    fn render(&self, frame: &mut ratatui::Frame<'_>, area: Rect);
    fn handle_key(&mut self, key: KeyEvent) -> Result<FormResult, String>;
}
