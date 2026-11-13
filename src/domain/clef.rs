//! Clef type

use std::fmt;

/// A clef says which pitch each line and space of the five-line staff stands for
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Clef {
    /// G clef: its curl wraps the second line from the bottom, which is G4
    #[default]
    Treble,
    /// F clef: its dots straddle the fourth line from the bottom, which is F3
    Bass,
}

impl Clef {
    /// Where the bottom line of the staff is on the diatonic scale: `octave * 7 + letter`, with
    /// letters counted C = 0 to B = 6. The treble staff starts on E4 (30) and the bass staff
    /// on G2 (18).
    pub fn bottom_line_diatonic_index(&self) -> i32 {
        match self {
            Clef::Treble => 4 * 7 + 2,
            Clef::Bass => 2 * 7 + 4,
        }
    }

    /// Staff position of the line the clef names, in steps above the bottom line (lines are
    /// two steps apart): the G line (2) for treble and the F line (6) for bass
    pub fn reference_line_steps(&self) -> i32 {
        match self {
            Clef::Treble => 2,
            Clef::Bass => 6,
        }
    }

    /// MIDI number of the note on the middle line: B4 (71) for treble and D3 (50) for bass
    pub fn middle_line_midi(&self) -> u8 {
        match self {
            Clef::Treble => 71,
            Clef::Bass => 50,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Clef::Treble => "treble",
            Clef::Bass => "bass",
        }
    }

    /// Read a clef from its name: `treble` or `g`, `bass` or `f` (any case)
    pub fn from_name(name: &str) -> Option<Clef> {
        match name.trim().to_lowercase().as_str() {
            "treble" | "g" => Some(Clef::Treble),
            "bass" | "f" => Some(Clef::Bass),
            _ => None,
        }
    }
}

impl fmt::Display for Clef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_treble_is_the_default() {
        assert_eq!(Clef::default(), Clef::Treble);
    }

    #[test]
    fn test_bottom_lines() {
        // E4 and G2 in scientific pitch notation
        assert_eq!(Clef::Treble.bottom_line_diatonic_index(), 4 * 7 + 2);
        assert_eq!(Clef::Bass.bottom_line_diatonic_index(), 2 * 7 + 4);
    }

    #[test]
    fn test_reference_lines() {
        // The G line is the second line, the F line the fourth line
        assert_eq!(Clef::Treble.reference_line_steps(), 2);
        assert_eq!(Clef::Bass.reference_line_steps(), 6);
    }

    #[test]
    fn test_reference_line_holds_the_named_note() {
        // Treble: G4 sits on the reference line. Bass: F3 does.
        let g4 = 4 * 7 + 4;
        assert_eq!(
            g4 - Clef::Treble.bottom_line_diatonic_index(),
            Clef::Treble.reference_line_steps()
        );
        let f3 = 3 * 7 + 3;
        assert_eq!(
            f3 - Clef::Bass.bottom_line_diatonic_index(),
            Clef::Bass.reference_line_steps()
        );
    }

    #[test]
    fn test_middle_lines() {
        assert_eq!(Clef::Treble.middle_line_midi(), 71); // B4
        assert_eq!(Clef::Bass.middle_line_midi(), 50); // D3
    }

    #[test]
    fn test_names() {
        assert_eq!(Clef::from_name("treble"), Some(Clef::Treble));
        assert_eq!(Clef::from_name("BASS"), Some(Clef::Bass));
        assert_eq!(Clef::from_name(" g "), Some(Clef::Treble));
        assert_eq!(Clef::from_name("F"), Some(Clef::Bass));
        assert_eq!(Clef::from_name("alto"), None);
        assert_eq!(Clef::Bass.to_string(), "bass");
    }
}
