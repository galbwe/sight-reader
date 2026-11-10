//! Domain model for music theory concepts

pub mod clef;
pub mod note;
pub mod interval;
pub mod key;
pub mod time;
pub mod measure;

pub use clef::Clef;
pub use note::{Note, NoteDuration};
pub use interval::Interval;
pub use crate::domain::key::{Key, KeyMode, KeySignature, Accidental};
pub use time::TimeSignature;
pub use measure::Measure;
