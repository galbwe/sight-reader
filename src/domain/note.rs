//! Note representation and note duration types

use std::fmt;

/// Represents a musical note with pitch and duration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Note {
    /// MIDI note number (0-127), where 60 is middle C
    pub midi_number: u8,
    /// Duration of the note
    pub duration: NoteDuration,
    /// Whether the note is dotted
    pub dotted: bool,
    /// Whether the note is part of a triplet
    pub is_triplet: bool,
}

impl Note {
    /// Create a new note from a MIDI number and duration
    pub fn new(midi_number: u8, duration: NoteDuration) -> Self {
        Note {
            midi_number,
            duration,
            dotted: false,
            is_triplet: false,
        }
    }

    /// Create a dotted note
    pub fn dotted(mut self) -> Self {
        self.dotted = true;
        self
    }

    /// Create a triplet note
    pub fn triplet(mut self) -> Self {
        self.is_triplet = true;
        self
    }

    /// Get the note name without octave
    pub fn name(&self) -> &'static str {
        const NAMES: &[&str] = &[
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        NAMES[(self.midi_number % 12) as usize]
    }

    /// Get the octave number
    pub fn octave(&self) -> i32 {
        (self.midi_number / 12 - 1) as i32
    }

    /// Get the note name with octave (scientific pitch notation)
    pub fn with_octave(&self) -> String {
        format!("{}{}", self.name(), self.octave())
    }

    /// Calculate frequency in Hz (A4 = 440 Hz)
    pub fn frequency(&self) -> f64 {
        440.0 * 2.0f64.powf((self.midi_number as f64 - 69.0) / 12.0)
    }

    /// Get the note's position relative to middle C (60)
    /// Positive = above middle C, Negative = below middle C
    pub fn offset_from_middle_c(&self) -> i32 {
        self.midi_number as i32 - 60
    }

    /// Check if this note is within a given range
    pub fn is_in_range(&self, min: u8, max: u8) -> bool {
        self.midi_number >= min && self.midi_number <= max
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.name(), self.octave())?;
        if self.is_triplet {
            write!(f, " (triplet)")?;
        }
        if self.dotted {
            write!(f, " (dotted)")?;
        }
        Ok(())
    }
}

/// Note duration types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteDuration {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
}

impl NoteDuration {
    /// Get the duration as a fraction of a whole note
    pub fn as_fraction(&self) -> (u32, u32) {
        match self {
            NoteDuration::Whole => (1, 1),
            NoteDuration::Half => (1, 2),
            NoteDuration::Quarter => (1, 4),
            NoteDuration::Eighth => (1, 8),
            NoteDuration::Sixteenth => (1, 16),
            NoteDuration::ThirtySecond => (1, 32),
            NoteDuration::SixtyFourth => (1, 64),
        }
    }

    /// Get duration as beats in 4/4 time
    pub fn beats_in_4_4(&self) -> f64 {
        match self {
            NoteDuration::Whole => 4.0,
            NoteDuration::Half => 2.0,
            NoteDuration::Quarter => 1.0,
            NoteDuration::Eighth => 0.5,
            NoteDuration::Sixteenth => 0.25,
            NoteDuration::ThirtySecond => 0.125,
            NoteDuration::SixtyFourth => 0.0625,
        }
    }

    /// Get a human-readable name for the duration
    pub fn name(&self) -> &'static str {
        match self {
            NoteDuration::Whole => "whole",
            NoteDuration::Half => "half",
            NoteDuration::Quarter => "quarter",
            NoteDuration::Eighth => "eighth",
            NoteDuration::Sixteenth => "sixteenth",
            NoteDuration::ThirtySecond => "thirty-second",
            NoteDuration::SixtyFourth => "sixty-fourth",
        }
    }
}

impl fmt::Display for NoteDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_creation() {
        let c4 = Note::new(60, NoteDuration::Quarter);
        assert_eq!(c4.name(), "C");
        assert_eq!(c4.octave(), 4);
        assert_eq!(c4.with_octave(), "C4");
    }

    #[test]
    fn test_note_frequencies() {
        let a4 = Note::new(69, NoteDuration::Quarter);
        assert!((a4.frequency() - 440.0).abs() < 0.01);

        let c4 = Note::new(60, NoteDuration::Quarter);
        assert!((c4.frequency() - 261.63).abs() < 0.01);
    }

    #[test]
    fn test_dotted_duration() {
        let c4 = Note::new(60, NoteDuration::Quarter).dotted();
        assert!(c4.dotted);
        // Check that dotted flag is set
    }

    #[test]
    fn test_triplet_duration() {
        let c4 = Note::new(60, NoteDuration::Quarter).triplet();
        assert!(c4.is_triplet);
    }
}
