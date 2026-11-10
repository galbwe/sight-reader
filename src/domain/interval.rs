//! Interval representation

use std::fmt;

/// Represents the distance between two notes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    /// Number of scale steps (1 = unison, 2 = second, etc.)
    pub scale_steps: i32,
    /// Number of semitones
    pub semitones: i32,
}

impl Interval {
    /// Create a new interval
    pub fn new(scale_steps: i32, semitones: i32) -> Self {
        Interval { scale_steps, semitones }
    }

    /// Create a unison interval
    pub fn unison() -> Self {
        Interval::new(1, 0)
    }

    /// Create a perfect unison
    pub fn perfect_unison() -> Self {
        Interval::unison()
    }

    /// Create a minor second (half step)
    pub fn minor_second() -> Self {
        Interval::new(2, 1)
    }

    /// Create a major second (whole step)
    pub fn major_second() -> Self {
        Interval::new(2, 2)
    }

    /// Create a minor third
    pub fn minor_third() -> Self {
        Interval::new(3, 3)
    }

    /// Create a major third
    pub fn major_third() -> Self {
        Interval::new(3, 4)
    }

    /// Create a perfect fourth
    pub fn perfect_fourth() -> Self {
        Interval::new(4, 5)
    }

    /// Create a tritone
    pub fn tritone() -> Self {
        Interval::new(4, 6)
    }

    /// Create a perfect fifth
    pub fn perfect_fifth() -> Self {
        Interval::new(5, 7)
    }

    /// Create a minor sixth
    pub fn minor_sixth() -> Self {
        Interval::new(6, 8)
    }

    /// Create a major sixth
    pub fn major_sixth() -> Self {
        Interval::new(6, 9)
    }

    /// Create a minor seventh
    pub fn minor_seventh() -> Self {
        Interval::new(7, 10)
    }

    /// Create a major seventh
    pub fn major_seventh() -> Self {
        Interval::new(7, 11)
    }

    /// Create an octave
    pub fn octave() -> Self {
        Interval::new(8, 12)
    }

    /// Check if this is a step (minor or major second)
    pub fn is_step(&self) -> bool {
        self.scale_steps == 2 && self.semitones <= 2
    }

    /// Check if this is a skip (minor third or larger)
    pub fn is_skip(&self) -> bool {
        self.scale_steps >= 3
    }

    /// Check if this is a leap (perfect fourth or larger)
    pub fn is_leap(&self) -> bool {
        self.scale_steps >= 4
    }

    /// Apply this interval to a note, returning the resulting note
    /// Note: This is a simplified version that works with diatonic intervals
    pub fn apply_to_note(&self, note_midi: u8) -> u8 {
        // This is a simplified implementation
        // In a full implementation, you'd need to consider key signature and accidentals
        note_midi.saturating_add(self.semitones as u8)
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let quality = self.determine_quality();
        let number = self.scale_steps;
        
        // Handle compound intervals
        let (number, octave_offset) = if number > 8 {
            (number - 7, (number - 1) / 7)
        } else {
            (number, 0)
        };

        let quality_str = match quality {
            "Perfect" if number == 1 => "unison",
            "Perfect" if number == 4 => "fourth",
            "Perfect" if number == 5 => "fifth",
            "Perfect" if number == 8 => "octave",
            "Minor" => match number {
                2 => "second",
                3 => "third",
                6 => "sixth",
                7 => "seventh",
                9 => "ninth",
                _ => "interval",
            },
            "Major" => match number {
                2 => "second",
                3 => "third",
                6 => "sixth",
                7 => "seventh",
                9 => "ninth",
                _ => "interval",
            },
            _ => "interval",
        };

        if octave_offset > 0 {
            write!(f, "{} {} ({} octaves)", quality_str, number, octave_offset)
        } else {
            write!(f, "{} {}", quality_str, number)
        }
    }
}

impl Interval {
    fn determine_quality(&self) -> &'static str {
        let semitones = self.semitones;
        let scale_steps = self.scale_steps;

        // Determine expected semitones for diatonic intervals
        let diatonic_semitones = match scale_steps {
            1 => 0,
            2 => 1,
            3 => 4,
            4 => 5,
            5 => 7,
            6 => 8,
            7 => 10,
            8 => 12,
            _ => 0,
        };

        if semitones == diatonic_semitones {
            "Perfect"
        } else if semitones == diatonic_semitones - 1 {
            "Minor"
        } else if semitones == diatonic_semitones + 1 {
            "Augmented"
        } else if semitones < diatonic_semitones - 1 {
            "Diminished"
        } else {
            "Augmented"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_creation() {
        let second = Interval::major_second();
        assert_eq!(second.scale_steps, 2);
        assert_eq!(second.semitones, 2);
    }

    #[test]
    fn test_interval_types() {
        assert!(Interval::major_second().is_step());
        assert!(Interval::minor_third().is_skip());
        assert!(Interval::perfect_fourth().is_leap());
    }

    #[test]
    fn test_interval_application() {
        let c4 = 60u8;
        let third = Interval::major_third();
        let e4 = third.apply_to_note(c4);
        assert_eq!(e4, 64); // C4 (60) + 4 semitones = E4 (64)
    }
}
