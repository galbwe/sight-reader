//! Configuration and selection types for exercise generation

use crate::domain::NoteDuration;

/// Configuration for which durations to include
#[derive(Debug, Clone)]
pub struct NoteDurationSelection {
    pub whole: DurationWeight,
    pub half: DurationWeight,
    pub quarter: DurationWeight,
    pub eighth: DurationWeight,
    pub sixteenth: DurationWeight,
    pub dotted_quarter: bool,
    pub dotted_eighth: bool,
    pub dotted_sixteenth: bool,
    pub triplet_quarter: bool,
    pub triplet_eighth: bool,
}

impl NoteDurationSelection {
    /// Create a default selection (all durations enabled with equal weighting)
    pub fn default() -> Self {
        NoteDurationSelection {
            whole: DurationWeight::Enabled(1),
            half: DurationWeight::Enabled(1),
            quarter: DurationWeight::Enabled(2),
            eighth: DurationWeight::Enabled(3),
            sixteenth: DurationWeight::Enabled(2),
            dotted_quarter: true,
            dotted_eighth: true,
            dotted_sixteenth: true,
            triplet_quarter: true,
            triplet_eighth: true,
        }
    }

    /// Create a selection with only simple durations (no dots, no triplets)
    pub fn simple_durations() -> Self {
        NoteDurationSelection {
            whole: DurationWeight::Enabled(1),
            half: DurationWeight::Enabled(1),
            quarter: DurationWeight::Enabled(2),
            eighth: DurationWeight::Enabled(2),
            sixteenth: DurationWeight::Enabled(1),
            dotted_quarter: false,
            dotted_eighth: false,
            dotted_sixteenth: false,
            triplet_quarter: false,
            triplet_eighth: false,
        }
    }

    /// Get valid durations that fit within a given time span (in beats)
    pub fn get_valid_durations(&self, max_duration: f64) -> Vec<NoteDuration> {
        let mut durations = Vec::new();
        
        if self.whole.weight() > 0 && max_duration >= 4.0 {
            durations.push(NoteDuration::Whole);
        }
        if self.half.weight() > 0 && max_duration >= 2.0 {
            durations.push(NoteDuration::Half);
        }
        if self.quarter.weight() > 0 && max_duration >= 1.0 {
            durations.push(NoteDuration::Quarter);
        }
        if self.eighth.weight() > 0 && max_duration >= 0.5 {
            durations.push(NoteDuration::Eighth);
        }
        if self.sixteenth.weight() > 0 && max_duration >= 0.25 {
            durations.push(NoteDuration::Sixteenth);
        }
        
        durations
    }

    /// Get the weight for a duration
    pub fn weight(&self, duration: NoteDuration) -> u32 {
        match duration {
            NoteDuration::Whole => self.whole.weight(),
            NoteDuration::Half => self.half.weight(),
            NoteDuration::Quarter => self.quarter.weight(),
            NoteDuration::Eighth => self.eighth.weight(),
            NoteDuration::Sixteenth => self.sixteenth.weight(),
            NoteDuration::ThirtySecond => 0,
            NoteDuration::SixtyFourth => 0,
        }
    }

    /// Check if dotted versions are enabled
    pub fn dotted_enabled(&self) -> bool {
        self.dotted_quarter || self.dotted_eighth || self.dotted_sixteenth
    }

    /// Check if triplets are enabled
    pub fn triplets_enabled(&self) -> bool {
        self.triplet_quarter || self.triplet_eighth
    }
}

/// Weight for a note duration (enabled with weight, or disabled)
#[derive(Debug, Clone, Copy)]
pub enum DurationWeight {
    Disabled,
    Enabled(u32),
}

impl DurationWeight {
    /// Get the weight value
    pub fn weight(&self) -> u32 {
        match self {
            DurationWeight::Disabled => 0,
            DurationWeight::Enabled(w) => *w,
        }
    }
}

/// Configuration for which intervals to include
#[derive(Debug, Clone)]
pub struct IntervalSelection {
    pub steps: bool,          // Minor and major seconds
    pub skips: bool,          // Thirds and larger
    pub leaps: bool,          // Fourth and larger
    pub unisons: bool,        // Same note repeated
    pub octave: bool,         // Octave jumps
    pub direction_bias: DirectionBias,
}

impl IntervalSelection {
    /// Create a default selection (all intervals enabled)
    pub fn default() -> Self {
        IntervalSelection {
            steps: true,
            skips: true,
            leaps: true,
            unisons: true,
            octave: true,
            direction_bias: DirectionBias::Balanced,
        }
    }

    /// Create a selection with only steps (for easier exercises)
    pub fn steps_only() -> Self {
        IntervalSelection {
            steps: true,
            skips: false,
            leaps: false,
            unisons: true,
            octave: false,
            direction_bias: DirectionBias::Balanced,
        }
    }

    /// Create a selection with steps and skips only
    pub fn steps_and_skips() -> Self {
        IntervalSelection {
            steps: true,
            skips: true,
            leaps: false,
            unisons: true,
            octave: false,
            direction_bias: DirectionBias::Balanced,
        }
    }

    /// Create a selection with larger intervals for more challenge
    pub fn larger_intervals() -> Self {
        IntervalSelection {
            steps: true,
            skips: true,
            leaps: true,
            unisons: true,
            octave: true,
            direction_bias: DirectionBias::LeaningUp,
        }
    }

    /// Get a direction based on the bias
    pub fn direction(&self) -> IntervalDirection {
        match self.direction_bias {
            DirectionBias::Balanced => {
                // Will be determined by random selection
                IntervalDirection::Random
            }
            DirectionBias::LeaningUp => IntervalDirection::Up,
            DirectionBias::LeaningDown => IntervalDirection::Down,
        }
    }
}

/// Direction bias for interval selection
#[derive(Debug, Clone, Copy)]
pub enum DirectionBias {
    Balanced,
    LeaningUp,
    LeaningDown,
}

/// Direction of an interval
#[derive(Debug, Clone, Copy)]
pub enum IntervalDirection {
    Up,
    Down,
    Random,
}

impl IntervalDirection {
    /// Get a direction based on the bias
    pub fn from_bias(bias: DirectionBias) -> Self {
        match bias {
            DirectionBias::Balanced => IntervalDirection::Random,
            DirectionBias::LeaningUp => IntervalDirection::Up,
            DirectionBias::LeaningDown => IntervalDirection::Down,
        }
    }
}
