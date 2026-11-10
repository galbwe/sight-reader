//! Time signature and beat division types

use std::fmt;

/// Represents a time signature (e.g., 4/4, 3/4, 6/8)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeSignature {
    /// Number of beats per measure
    pub numerator: u8,
    /// The note value that gets one beat
    pub denominator: u8,
}

impl TimeSignature {
    /// Create a new time signature
    pub fn new(numerator: u8, denominator: u8) -> Self {
        TimeSignature { numerator, denominator }
    }

    /// Create 4/4 time (common time)
    pub fn common_time() -> Self {
        TimeSignature::new(4, 4)
    }

    /// Create 3/4 time
    pub fn three_four() -> Self {
        TimeSignature::new(3, 4)
    }

    /// Create 6/8 time
    pub fn six_eight() -> Self {
        TimeSignature::new(6, 8)
    }

    /// Create 2/4 time
    pub fn two_four() -> Self {
        TimeSignature::new(2, 4)
    }

    /// Create 5/4 time
    pub fn five_four() -> Self {
        TimeSignature::new(5, 4)
    }

    /// Get the number of beats per measure
    pub fn beats_per_measure(&self) -> u8 {
        self.numerator
    }

    /// Get the beat value (denominator as a fraction of a whole note)
    pub fn beat_value(&self) -> f64 {
        match self.denominator {
            1 => 1.0,
            2 => 0.5,
            4 => 0.25,
            8 => 0.125,
            16 => 0.0625,
            32 => 0.03125,
            _ => 0.25, // default to quarter note
        }
    }

    /// Check if this is a simple time signature (2, 3, or 4 numerator)
    pub fn is_simple(&self) -> bool {
        self.numerator <= 4
    }

    /// Check if this is a compound time signature (6, 9, or 12 numerator)
    pub fn is_compound(&self) -> bool {
        [6, 9, 12].contains(&self.numerator)
    }

    /// Check if this is an odd time signature
    pub fn is_odd(&self) -> bool {
        self.numerator > 4 && !self.is_compound()
    }

    /// Calculate total beats in a measure (returns numerator)
    pub fn total_beats(&self) -> f64 {
        self.numerator as f64
    }
}

impl fmt::Display for TimeSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}

/// Beat division types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BeatDivision {
    /// Two notes per beat (duple)
    Duple,
    /// Three notes per beat (triple)
    Triple,
    /// Four notes per beat (quadruple)
    Quadruple,
    /// Six notes per beat (compound duple)
    CompoundDuple,
    /// Nine notes per beat (compound triple)
    CompoundTriple,
    /// Twelve notes per beat (compound quadruple)
    CompoundQuadruple,
}

impl BeatDivision {
    /// Determine the beat division from a time signature
    pub fn from_time_signature(ts: TimeSignature) -> Self {
        match ts.numerator {
            2 | 3 | 4 => {
                if ts.denominator == 8 {
                    // Could be compound
                    match ts.numerator {
                        6 => BeatDivision::CompoundDuple,
                        9 => BeatDivision::CompoundTriple,
                        12 => BeatDivision::CompoundQuadruple,
                        _ => BeatDivision::Duple, // simple with eighth beat
                    }
                } else {
                    BeatDivision::Duple
                }
            }
            6 => BeatDivision::CompoundDuple,
            9 => BeatDivision::CompoundTriple,
            12 => BeatDivision::CompoundQuadruple,
            _ => BeatDivision::Duple,
        }
    }

    /// Get the number of sub-beats per beat
    pub fn sub_beats(&self) -> u8 {
        match self {
            BeatDivision::Duple => 2,
            BeatDivision::Triple => 3,
            BeatDivision::Quadruple => 4,
            BeatDivision::CompoundDuple => 3,
            BeatDivision::CompoundTriple => 3,
            BeatDivision::CompoundQuadruple => 3,
        }
    }

    /// Get a human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            BeatDivision::Duple => "duple",
            BeatDivision::Triple => "triple",
            BeatDivision::Quadruple => "quadruple",
            BeatDivision::CompoundDuple => "compound duple",
            BeatDivision::CompoundTriple => "compound triple",
            BeatDivision::CompoundQuadruple => "compound quadruple",
        }
    }
}

impl fmt::Display for BeatDivision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_signature_creation() {
        let ts = TimeSignature::new(4, 4);
        assert_eq!(ts.numerator, 4);
        assert_eq!(ts.denominator, 4);
        assert_eq!(ts.to_string(), "4/4");
    }

    #[test]
    fn test_beat_value() {
        let ts = TimeSignature::new(4, 4);
        assert_eq!(ts.beat_value(), 0.25); // quarter note
        
        let ts = TimeSignature::new(6, 8);
        assert_eq!(ts.beat_value(), 0.125); // eighth note
    }

    #[test]
    fn test_compound_time() {
        assert!(TimeSignature::new(6, 8).is_compound());
        assert!(!TimeSignature::new(4, 4).is_compound());
    }

    #[test]
    fn test_beat_division() {
        let ts = TimeSignature::new(6, 8);
        let division = BeatDivision::from_time_signature(ts);
        assert_eq!(division, BeatDivision::CompoundDuple);
    }
}
