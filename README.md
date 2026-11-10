# Sight Reading Trainer

A Rust application for generating sight reading exercises for musicians.

## Reading the ASCII Output

The application displays exercises using a simplified ASCII notation. Here's how to interpret the output:

### Basic Structure

Each measure is displayed as:
```
Key: 1 #    Time: 4/4
--------------------------------------------------
--------------------------------------------------
--------------------------------------------------
--------------------------------------------------
--------------------------------------------------
NF#3    NA#3    NG#4    NC4    ND4
--------------------------------------------------
```

### Key Signature

- **`Key: 1 #`** - One sharp in the key signature
- **`Key: 2 b`** - Two flats in the key signature
- **`Key: C Major / A Minor`** - No accidentals (C major or A minor)

The accidentals shown are for the **key signature**, not for each individual note.

### Staff Lines

Five hyphen lines (`-----`) represent the staff lines. Notes are displayed above these lines, with their position indicating pitch (higher notes appear closer to the top lines).

### Notes

Each note is displayed as: `AccidentalNoteOctave`

Examples:
- **`NF#3`** - F sharp in octave 3
- **`NG#4`** - G sharp in octave 4
- **`NA#3`** - A sharp in octave 3
- **`NC#4`** - C sharp in octave 4

Note format breakdown:
| Component | Example | Description |
|-----------|---------|-------------|
| Note head | `N` | Note head symbol (simplified: `N` for all durations) |
| Accidental | `F#` | Note name with accidental from key signature |
| Octave | `3` | Scientific pitch notation octave number |

### Key Signature vs. Individual Accidentals

The displayed notes show the **key signature accidental** applied to each note:

- If a note is **in the key**, it shows the key's accidental (e.g., `NF#` in G major)
- If a note is **chromatic** (outside the key), it shows the chromatic accidental (e.g., `NC#` in G major when C natural appears)

### Duration Indicators

Currently, the ASCII display shows a simplified `N` for all note durations. Duration information is stored in the domain model but not yet visually represented in the ASCII output. The generator respects your duration selection when creating exercises.

### Example Walkthrough

```
Key: 1 #    Time: 4/4
--------------------------------------------------
NF#3    NA#3    NG#4    NC#4    ND4    NE4
```

- **Time: 4/4** - Four beats per measure, quarter note = one beat
- **Key: 1 #** - G major (one sharp = F#)
- **Notes**:
  - `NF#3` - F# (key signature) in octave 3
  - `NA#3` - A# (chromatic) in octave 3
  - `NG#4` - G# (chromatic) in octave 4
  - `NC#4` - C# (chromatic) in octave 4
  - `ND4` - D (natural, in key) in octave 4
  - `NE4` - E (natural, in key) in octave 4

### Reading Tips

1. **Identify the key** first from the key signature
2. **Read each note** from left to right, following the staff
3. **Watch for accidentals** - they indicate whether the note follows the key signature or is chromatic
4. **Practice by singing** the note names aloud as you read

## Features

- Generate random sight reading exercises with configurable parameters
- Multiple key signatures support
- Configurable octave range
- Various note durations (whole, half, quarter, eighth, sixteenth, dotted, triplets)
- Interval selection (steps, skips, leaps)
- ASCII art notation display
- PDF export
- Simple CLI interface

## Project Structure

The project is organized with a clean separation between domain logic and presentation:

```
src/
├── domain/          # Core music theory concepts
│   ├── mod.rs
│   ├── note.rs      # Note representation
│   ├── interval.rs  # Interval types and calculations
│   ├── key.rs       # Key signatures and modes
│   ├── time.rs      # Time signatures and beat divisions
│   ├── measure.rs   # Measure structure
│   └── clef.rs      # Treble and bass clef
├── generator/       # Exercise generation
│   ├── mod.rs       # Generator and configuration
│   ├── config.rs    # User configuration types
│   ├── melody.rs    # Picks pitches from the key's scale
│   ├── rules.rs     # Weights for moves through the scale
│   └── musical_phrase.rs
├── presentation/    # Display layer
│   ├── mod.rs
│   ├── ascii.rs     # ASCII art notation renderer
│   ├── pdf.rs       # PDF notation renderer
│   ├── beaming.rs   # Beam grouping and beam geometry
│   ├── spelling.rs  # Note spelling and accidentals
│   ├── music_glyphs.rs  # Generated clef, accidental and digit shapes (Bravura)
│   └── tui.rs       # TUI renderer (stub for future)
└── main.rs          # CLI entry point
```

## Usage

```bash
# Show help
cargo run -- --help

# Generate 8 measures in C major
cargo run -- -m 8 -k C

# Generate 16 measures in G major with 3 octaves
cargo run -- -m 16 -k G -o 3

# Generate with quarter note durations only
cargo run -- -m 8 -k C -d quarter

# Generate with only stepwise intervals
cargo run -- -m 8 -k C -i steps

# Generate and export to PDF
cargo run -- -m 8 -k G --format pdf --output exercise.pdf

# The same in bass clef
cargo run -- -m 8 -k F --clef bass --format pdf --output bass.pdf
```

## Options

- `-m, --measures <COUNT>`: Number of measures to generate (default: 4)
- `-k, --key <KEY>`: Key (C, C#, D, D#, E, F, F#, G, G#, A, A#, B)
- `-c, --clef <CLEF>`: Clef, treble or bass (default: treble). The clef also sets the range of notes: C4 to B5 for treble and C2 to B3 for bass, with the default of 2 octaves.
- `-o, --octaves <COUNT>`: Number of octaves to cover, 1 to 10 (default: 2). A value that is not a whole number in that range is an error.
- `-d, --durations <TYPE>`: Duration types (simple, quarter, dotted, default: all)
- `-i, --intervals <TYPE>`: Interval types (steps, steps-skips, large, default: all)
- `-f, --format <FORMAT>`: Output format (ascii, pdf, default: ascii)
- `--output <FILE>`: Output file path (for PDF export, default: `exercise.pdf`). This option has no short form: `-o` is the octave count.
- `-h, --help`: Show help message

## Current Status

The initial version is functional with:
- Complete domain model for music theory concepts
- Guided random exercise generation: melodies move up and down through the notes of the key,
  prefer steps, and recover from leaps
- ASCII art notation display
- PDF export with treble or bass clef, key signature (sharps and flats), time signature, noteheads, stems,
  beams (grouped by the beat), flags, dots, ledger lines, accidentals and key-aware note spelling,
  laid out over several measures and pages
- CLI interface

## Future Enhancements

- Improved ASCII notation with better visual representation
- Full treble/bass staff notation
- Support for grand staff (treble + bass clefs)
- PDF export (implemented)
- Interactive TUI with keyboard shortcuts
- Audio playback of generated exercises
- Web application frontend

## License

MIT

The clef, hollow notehead, sharp, flat, natural and time signature digit shapes used in the PDF output
(`src/presentation/music_glyphs.rs`) are derived from the
[Bravura](https://github.com/steinbergmedia/bravura) music font,
Copyright (c) 2015 Steinberg Media Technologies GmbH, licensed under the
SIL Open Font License 1.1 (see `LICENSES/Bravura-OFL.txt`).
