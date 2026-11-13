//! ASCII art notation renderer

use crate::domain::{Accidental, KeySignature, Measure, Note, NoteDuration};

/// Renderer for ASCII art notation
pub struct AsciiNotationRenderer {
    staff_lines: u8,
    show_clef: bool,
    show_key_signature: bool,
    show_time_signature: bool,
}

impl AsciiNotationRenderer {
    pub fn new() -> Self {
        AsciiNotationRenderer {
            staff_lines: 5,
            show_clef: true,
            show_key_signature: true,
            show_time_signature: true,
        }
    }

    pub fn with_staff_lines(self, lines: u8) -> Self {
        Self {
            staff_lines: lines,
            ..self
        }
    }

    pub fn without_clef(self) -> Self {
        Self {
            show_clef: false,
            ..self
        }
    }

    pub fn without_key_signature(self) -> Self {
        Self {
            show_key_signature: false,
            ..self
        }
    }

    pub fn without_time_signature(self) -> Self {
        Self {
            show_time_signature: false,
            ..self
        }
    }
}

impl Default for AsciiNotationRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::presentation::NotationRenderer for AsciiNotationRenderer {
    fn render_measure(&self, measure: &Measure) -> String {
        let mut output = String::new();

        // Key signature
        if self.show_key_signature {
            let key_sig = &measure.key_signature;
            if key_sig.accidental_count != 0 {
                let (count, symbol) = if key_sig.accidental_count > 0 {
                    (key_sig.accidental_count as u8, "#")
                } else {
                    (-key_sig.accidental_count as u8, "b")
                };
                output.push_str(&format!("Key: {} {}    ", count, symbol));
            } else {
                output.push_str("Key: C Major / A Minor    ");
            }
        }

        // Time signature
        if self.show_time_signature {
            output.push_str(&format!("Time: {}    ", measure.time_signature));
        }

        output.push('\n');

        // Staff lines
        for _ in 0..self.staff_lines {
            output.push_str(&"-----".repeat(10));
            output.push('\n');
        }

        // Notes
        for note in &measure.notes {
            output.push_str(&self.render_note(note, &measure.key_signature));
            output.push_str("    ");
        }

        output.push('\n');

        // Bottom staff line
        output.push_str(&"-----".repeat(10));

        output
    }

    fn render_note(&self, note: &Note, key_sig: &KeySignature) -> String {
        let name = note.name();
        let octave = note.octave();

        // Determine staff position based on pitch
        let _position = self.note_position(note.midi_number);

        // Create the note head
        let note_head = self.note_head(note.duration, note.dotted, note.is_triplet);

        // Determine accidental
        let accidental = self.note_accidental(note, key_sig);

        format!("{}{}{}{}", accidental, note_head, name, octave)
    }
}

impl AsciiNotationRenderer {
    fn note_position(&self, midi: u8) -> i32 {
        // Position on the staff (0 = bottom line, 8 = top line)
        // This is a simplified calculation
        let relative = (midi as i32 - 60) % 14; // C4 is reference
        if relative < 0 {
            relative + 14
        } else {
            relative
        }
    }

    fn note_head(&self, _duration: NoteDuration, _dotted: bool, _triplet: bool) -> &'static str {
        "N" // Simplified
    }

    fn note_accidental(&self, note: &Note, key_sig: &KeySignature) -> &'static str {
        // Check if note has an accidental in the key signature
        let note_step = (note.midi_number % 12) as i32;
        let step_in_scale = self.note_step_in_scale(note_step);

        let accidental = key_sig.get_accidental_for_step(step_in_scale);

        match accidental {
            Accidental::Natural => "",
            Accidental::Sharp => "#",
            Accidental::Flat => "b",
            Accidental::DoubleSharp => "##",
            Accidental::DoubleFlat => "bb",
        }
    }

    fn note_step_in_scale(&self, note_step: i32) -> i32 {
        // Map note step (0-11) to scale step (0-6)
        const CHROMATIC_TO_DIATONIC: &[i32] = &[0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
        CHROMATIC_TO_DIATONIC[note_step as usize % 12]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_position() {
        let renderer = AsciiNotationRenderer::new();
        assert_eq!(renderer.note_position(60), 0); // C4
    }

    #[test]
    fn test_note_head() {
        let renderer = AsciiNotationRenderer::new();
        assert_eq!(renderer.note_head(NoteDuration::Whole, false, false), "N");
        assert_eq!(renderer.note_head(NoteDuration::Quarter, false, false), "N");
    }
}
