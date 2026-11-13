//! TUI renderer (stub for future implementation)

use crate::domain::{KeySignature, Measure, Note};

/// TUI renderer for terminal UI
pub struct TuiRenderer;

impl TuiRenderer {
    pub fn new() -> Self {
        TuiRenderer
    }
}

impl Default for TuiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::presentation::NotationRenderer for TuiRenderer {
    fn render_measure(&self, _measure: &Measure) -> String {
        String::new()
    }

    fn render_note(&self, _note: &Note, _key_sig: &KeySignature) -> String {
        String::new()
    }
}
