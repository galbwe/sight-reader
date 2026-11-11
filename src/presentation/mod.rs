//! Presentation layer for displaying music notation

pub mod ascii;
mod beaming;
pub mod midi;
mod music_glyphs;
mod spelling;
pub mod pdf;
pub mod tui;
pub mod wav;

pub use ascii::AsciiNotationRenderer;
pub use midi::MidiRenderer;
pub use pdf::PdfNotationRenderer;
pub use wav::WavRenderer;
// pub use tui::TuiRenderer;

use crate::domain::{Measure, Note};
use crate::domain::KeySignature;

/// Trait for rendering musical notation
pub trait NotationRenderer {
    /// Render a single measure
    fn render_measure(&self, measure: &Measure) -> String;
    
    /// Render multiple measures
    fn render_measures(&self, measures: &[Measure]) -> String {
        measures.iter()
            .map(|m| self.render_measure(m))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
    
    /// Render a single note
    fn render_note(&self, note: &Note, key_sig: &KeySignature) -> String;
}

/// Get a renderer based on type
pub fn get_renderer(renderer_type: &str) -> Box<dyn NotationRenderer> {
    match renderer_type {
        "ascii" => Box::new(AsciiNotationRenderer::new()),
        "pdf" => Box::new(PdfNotationRenderer::new()),
        _ => Box::new(AsciiNotationRenderer::new()),
    }
}
