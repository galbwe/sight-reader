//! Domain model for music theory concepts

pub mod clef;
pub mod interval;
pub mod key;
pub mod measure;
pub mod note;
pub mod time;

pub use crate::domain::key::{Accidental, Key, KeyMode, KeySignature};
pub use clef::Clef;
pub use interval::Interval;
pub use measure::Measure;
pub use note::{Note, NoteDuration};
pub use time::TimeSignature;
