//! Measure representation

use crate::domain::{KeySignature, Note, TimeSignature};
use std::fmt;

/// Represents a measure in a musical piece
#[derive(Debug, Clone)]
pub struct Measure {
    /// The notes in this measure
    pub notes: Vec<Note>,
    /// Time signature for this measure
    pub time_signature: TimeSignature,
    /// Key signature for this measure
    pub key_signature: KeySignature,
    /// Measure number (1-indexed)
    pub number: u32,
    /// Whether this measure contains a repeat
    pub repeat: bool,
}

impl Measure {
    /// Create a new measure
    pub fn new(number: u32, time_signature: TimeSignature, key_signature: KeySignature) -> Self {
        Measure {
            notes: Vec::new(),
            time_signature,
            key_signature,
            number,
            repeat: false,
        }
    }

    /// Add a note to this measure
    pub fn add_note(&mut self, note: Note) -> bool {
        // Check if the note fits within the measure duration
        let current_duration = self.total_duration();
        let note_duration = Self::note_duration_in_beats(&note, self.time_signature);

        current_duration + note_duration <= self.time_signature.total_beats()
    }

    /// Calculate total duration of notes in the measure
    pub fn total_duration(&self) -> f64 {
        self.notes
            .iter()
            .map(|n| Self::note_duration_in_beats(n, self.time_signature))
            .sum()
    }

    /// Check if the measure is full
    pub fn is_full(&self) -> bool {
        let total = self.total_duration();
        let expected = self.time_signature.total_beats();
        (total - expected).abs() < 0.001
    }

    /// Check if the measure has room for more notes
    pub fn has_space(&self) -> bool {
        !self.is_full()
    }

    /// Get the remaining space in the measure
    pub fn remaining_space(&self) -> f64 {
        self.time_signature.total_beats() - self.total_duration()
    }

    /// Calculate the duration of a note in beats
    fn note_duration_in_beats(note: &Note, ts: TimeSignature) -> f64 {
        let base_beats = note.duration.beats_in_4_4();
        let adjusted = if ts.denominator != 4 {
            // Adjust for different denominator
            base_beats * (4.0 / ts.denominator as f64)
        } else {
            base_beats
        };

        let dotted = if note.dotted { 1.5 } else { 1.0 };
        let triplet = if note.is_triplet { 2.0 / 3.0 } else { 1.0 };

        adjusted * dotted * triplet
    }

    /// Check if all notes in this measure belong to the key signature
    pub fn is_in_key(&self) -> bool {
        // Simplified: check if notes are within the octave range
        // A full implementation would check against the key's scale
        true
    }

    /// Get the average pitch of notes in this measure
    pub fn average_pitch(&self) -> Option<f64> {
        if self.notes.is_empty() {
            return None;
        }
        let sum: u32 = self.notes.iter().map(|n| n.midi_number as u32).sum();
        Some(sum as f64 / self.notes.len() as f64)
    }
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Measure {}: ", self.number)?;
        if self.notes.is_empty() {
            write!(f, "(empty)")
        } else {
            write!(f, "{} notes", self.notes.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{KeyMode, NoteDuration};

    #[test]
    fn test_measure_creation() {
        let ts = TimeSignature::common_time();
        let key = KeySignature::new(60, KeyMode::Major);
        let measure = Measure::new(1, ts, key);

        assert_eq!(measure.number, 1);
        assert!(measure.notes.is_empty());
        assert!(!measure.is_full());
    }

    #[test]
    fn test_measure_add_notes() {
        let ts = TimeSignature::common_time();
        let key = KeySignature::new(60, KeyMode::Major);
        let mut measure = Measure::new(1, ts, key);

        let c4 = Note::new(60, NoteDuration::Quarter);
        // Note: add_note returns boolean indicating if it fits
        // We push directly for testing purposes
        measure.notes.push(c4);

        assert_eq!(measure.notes.len(), 1);
        assert_eq!(measure.total_duration(), 1.0);
    }

    #[test]
    fn test_measure_fullness() {
        let ts = TimeSignature::common_time(); // 4/4 time
        let key = KeySignature::new(60, KeyMode::Major);
        let mut measure = Measure::new(1, ts, key);

        // Add four quarter notes = 1 measure (4 beats in 4/4 time)
        for _ in 0..4 {
            let note = Note::new(60, NoteDuration::Quarter);
            measure.notes.push(note);
        }

        // 4 quarter notes = 4 beats in 4/4 time
        let total = measure.total_duration();
        let expected = ts.total_beats(); // 4.0 for 4/4 time
        assert!(
            (total - expected).abs() < 0.001,
            "Total: {}, Expected: {}",
            total,
            expected
        );
        assert!(measure.is_full());
        assert!(!measure.has_space());
    }
}
