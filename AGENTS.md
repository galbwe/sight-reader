# Agents.md

This file provides guidance for AI agents working on this sight-reading trainer project. Please read this before making changes to ensure consistency and maintainability.

## Code Organization

The project follows a clean architecture pattern with clear separation of concerns:

### Module Hierarchy

```
sight-reading/
├── src/
│   ├── domain/          # Core domain model (pure data structures)
│   ├── generator/       # Exercise generation logic
│   ├── presentation/    # Output/rendering layer
│   └── main.rs          # CLI entry point
```

### Domain Model (`src/domain/`)

Contains the pure data structures representing music theory concepts. These types should be:

- **Stateless**: No business logic, only data storage and basic accessors
- **Serializable**: Should implement `Clone`, `Copy`, `Debug`, `PartialEq`
- **Independent**: No dependencies on generator or presentation modules
- **Well-tested**: All types have unit tests in their respective files

**Current modules:**
- `note.rs` - Note, NoteDuration types
- `interval.rs` - Interval type with music theory calculations
- `key.rs` - Key, KeyMode, KeySignature, Accidental types
- `time.rs` - TimeSignature, BeatDivision types
- `measure.rs` - Measure type representing a single measure
- `clef.rs` - Clef (treble or bass): where the staff's lines are on the diatonic scale

### Generator (`src/generator/`)

Contains the logic for generating exercises based on configuration:

- `mod.rs` - Main `ExerciseGenerator` and `GeneratorConfig`
- `config.rs` - User configuration types (`NoteDurationSelection`, `IntervalSelection`)
- `melody.rs` - `Melody`: picks pitches from the key's scale (see [Melody Generation](#melody-generation))
- `rules.rs` - `GenerationRules`: weights the possible moves through the scale
- `musical_phrase.rs` - `MusicalPhraseGenerator` for phrase-based generation (uses `Melody` too)

**Design principle:** Generators consume domain types and produce domain types (measures). They should not depend on presentation.

### Presentation (`src/presentation/`)

Contains rendering logic for displaying exercises:

- `mod.rs` - `NotationRenderer` trait for output abstraction
- `ascii.rs` - ASCII art notation renderer
- `pdf.rs` - PDF notation renderer built on `lopdf` (see [PDF Rendering](#pdf-rendering))
- `beaming.rs` - Which notes are beamed together and how beams and stems are laid out. Pure, no PDF code.
- `spelling.rs` - Note spelling and accidentals. Pure, no PDF code.
- `music_glyphs.rs` - Generated clef, accidental and digit outlines. Do not edit by hand.
- `tui.rs` - Stub for future TUI implementation

**Design principle:** Presentation modules should only consume domain types. They should not contain any generation logic.

### Entry Point (`src/main.rs`)

- Parses command-line arguments
- Creates generator configuration
- Generates exercises
- Renders output

## Music Theory Concepts

### Notes

- **MIDI numbers**: Notes are represented as MIDI note numbers (0-127), where 60 is middle C (C4)
- **Scientific pitch notation**: Format is note name + octave (e.g., C4, G#3, Ab5)
- **Note durations**: Whole, Half, Quarter, Eighth, Sixteenth, Thirty-second, Sixty-fourth
- **Modifiers**: Dotted notes (1.5x duration), triplets (2/3 duration)

### Intervals

- **Scale steps**: Distance in diatonic scale degrees (1 = unison, 2 = second, etc.)
- **Semitones**: Distance in chromatic semitones
- **Interval types**: Perfect, Major, Minor, Augmented, Diminished
- **Common intervals**: Unison, Second, Third, Fourth, Fifth, Sixth, Seventh, Octave

### Key Signatures

- **Major keys**: Follow circle of fifths (C=0, G=1, D=2, A=3, E=4, B=5, F#=6, C#=7 sharps)
- **Flat keys**: F=1, Bb=2, Eb=3, Ab=4, Db=5, Gb=6, Cb=7 flats
- **Relative minors**: A minor (relative to C major), E minor (relative to G major), etc.
- **Enharmonic equivalence**: C# and Db are different keys with different signatures

### Time Signatures

- **Numerator**: Beats per measure
- **Denominator**: Note value that gets one beat
- **Common time**: 4/4 (four quarter notes per measure)
- **Compound time**: 6/8, 9/8, 12/8 (each beat subdivides into three)

### Guided Randomness

Exercises use guided randomness rather than pure random generation:

1. **Stepwise motion preferred**: Intervals of 2nd/3rd are more common than leaps
2. **Limit octave range**: Notes stay within user-specified octave range
3. **Measure completion**: Notes must fit exactly within time signature
4. **Key consistency**: Every note belongs to the key's scale, so exercises have no accidentals. Implemented by `Melody`.

### Command Line

Parsing is in `src/main.rs`. `parse_args` returns `Result<Command, String>` (`Command::Run(Options)` or `Command::Help`) and never exits the process by itself, so it can be unit tested. `main` prints the error and exits with status 1.

- **`-o` means `--octaves`, and only that.** `--output` has **no short flag**. The two used to share `-o`: the octaves branch matched first, so `-o exercise.pdf` failed to parse as a number and was silently dropped, and the file was written as `exercise.pdf` regardless. `-o` was kept for octaves because every example in the README already used it that way. If you want a short flag for the output file, pick a different letter (not `-o`), and add it to `print_help` too.
- **Octave values are validated** (`parse_octaves`): a whole number from 1 to 10 (`MAX_OCTAVES`), otherwise an error. If the bad value contains letters, the message says `To name the output file, use --output <value>`.
- **No two options may share a short flag.** A test (`test_help_text_gives_o_to_octaves_only`) reads the help text and fails if they do. Keep `print_help` in step with the match arms in `parse_args`.
- Flags that need a value report `<flag> requires a value`, naming the long flag.

### Melody Generation

Implemented by `Melody` (`src/generator/melody.rs`) and `GenerationRules` (`src/generator/rules.rs`). The generator used to add raw semitones to the previous note and never looked at the key. The old result was chromatic exercises, melodies that only climbed, and a first note on F sharp.

- **Pitches come from the scale.** `scale_notes(key, min, max)` lists every note of the key's scale inside the range, lowest first. The melody is a position in that list, and a move of N entries is a move of N scale steps. So every note is in the key by construction, and a 2-step move is always a third, whatever the key.
- **Moves go both ways.** A move is a signed number of scale steps: 0 (repeat), 1 (second), 2 (third), 3 (fourth), 4 to 6 (fifth, sixth, seventh) and 7 (octave). `IntervalSelection` decides which sizes are allowed: `unisons` is 0, `steps` is 1, `skips` is 2 and 3, `leaps` is 4 to 6, and `octave` is 7. `direction_bias` doubles the weight of upward or downward moves.
- **Weights** (`BASE_WEIGHTS` in `rules.rs`): steps 16, thirds 8, fourths 4, fifths 3, sixths 2, sevenths 1, octaves 2, repeats 2. Then:
  - **Recover from a leap.** After a leap of a fourth or more, a step back in the opposite direction is doubled, carrying on the same way is halved, and another leap is cut to a quarter.
  - **Pull toward the middle.** A factor that is highest in the middle of the range and lowest at its edges (4 to 4 plus half the scale length), so melodies wander across the range instead of settling at one end. The edges stay reachable.
- **Start and continuity:** the first note is the tonic nearest the middle of the range. Each note, including the first of every measure, continues from the previous note. The generator keeps one `history` of pitches across all measures.
- **Randomness is isolated:** `candidate_moves` and `pick_move` are deterministic and take a `roll`, so they are tested exactly. `Melody::next_note` calls `rand::random`, and its tests check properties over hundreds of notes: in key, in range, both directions, mostly small moves. They were run 100 times in a row without a failure. If you add a statistical test, check it the same way.
- **Octave numbering:** the CLI's `--octaves` uses scientific pitch notation, where octave 4 starts at middle C (MIDI 60). The default of two octaves is C4 to B5 for treble clef and C2 to B3 for bass clef, which fit those staves. `OctaveRange` itself stores MIDI octaves (MIDI number divided by 12), which is one lower, so pass it MIDI numbers (use `octave_midi_range` in `main.rs`, which also takes the clef). The old code passed `octave * 12`, which put the default range at C3 to B4, mostly below the staff.
- **`Key::scale()`** returns the **pitch classes** of the scale starting from the tonic (G major is `[7, 9, 11, 0, 2, 4, 6]`). It used to return the same C major offsets for every key, which made `contains_note` wrong everywhere but C major.

**Not done:**
- Dotted notes and triplets are never generated. `NoteDurationSelection` has `dotted_*` and `triplet_*` flags, but `get_valid_durations` ignores them, so no dotted note appears in an exercise even though the PDF renderer can draw dots.
- No phrase shape (rise and fall, cadence). A melody may end on any scale note. A closing tonic would be a natural addition.
- No rhythm rules: durations are picked at random from what fits. Note values like a whole note after eighths are possible, and the generator does not try to start strong beats on longer notes.
- Minor keys use the natural minor scale. There is no raised leading tone.
- `Interval::apply_to_note` and `GenerationRules::recommended_intervals` are no longer used by the generator.
- **CLI leniency:** `--measures`, `--key`, `--durations`, `--intervals` and `--format` still fall back to a default when given a value they do not recognise, for example `-k H` silently means C major and `-m abc` means 4 measures. `--format` with an unknown value is the exception and exits with an error later. Only `--octaves` and `--clef` reject bad values.

## Design Decisions

### 1. MIDI Number Representation

**Decision**: Use MIDI note numbers (0-127) as the internal representation for notes.

**Reasoning**: 
- Standard, unambiguous representation
- Easy arithmetic for interval calculations
- Well-established ecosystem

**Trade-off**: Less intuitive for beginners, but this is an internal representation.

### 2. Separation of Domain and Generation

**Decision**: Domain types contain only data, not logic. Generation is in separate modules.

**Reasoning**:
- Testability: Domain types can be tested independently
- Flexibility: Multiple generation strategies can be implemented
- Reusability: Domain types can be used by presentation without generation dependencies

### 3. ASCII as Initial Presentation Layer

**Decision**: Start with ASCII art, designed to be replaced by web/TUI later.

**Reasoning**:
- Fast prototyping without external dependencies
- Clean abstraction: `NotationRenderer` trait decouples logic from output
- Future-proof: Can add `WebRenderer`, `TuiRenderer`, `PdfRenderer` implementations

### 4. CLI as Initial Interface

**Decision**: Start with command-line interface, designed to be replaced by web UI.

**Reasoning**:
- Fast development cycle for testing domain logic
- Can be called from other tools/scripts
- Easy to test with shell scripts
- Future web UI can use the same generator and domain types

### 5. Major Key Support Only (Initially)

**Decision**: Focus on major keys, minor key support limited.

**Reasoning**:
- Major keys are most common for sight-reading exercises
- Simpler implementation allows faster iteration
- Minor key support can be added incrementally

### 6. Guided vs. Pure Random

**Decision**: Use guided randomness with musical rules.

**Reasoning**:
- Pure random exercises are often musically awkward
- Stepwise motion is more natural for sight-reading
- Users can still get variety within musical constraints

## PDF Rendering

The PDF renderer (`src/presentation/pdf.rs`) writes raw PDF content-stream operators with `lopdf`. No PDF layout engine sits between our code and the page, so small mistakes in operator arguments produce output that is wildly wrong but still valid PDF. Nothing reports an error.

### Lessons Learned (Debugging History)

1. **`Tm` operand order (root cause of the "diagonal lines" bug).**
   `Tm` takes six operands `a b c d e f`. `a b c d` are scale/rotation/skew and **`e f` is the position**. The code was emitting `x y 1 0 0 1 Tm`, which scaled glyphs roughly 150x by 680x and placed them at (0, 1). Every glyph became a long diagonal smear starting at the bottom-left corner of the page, with thicker spots where the glyph shapes were. The correct form is `1 0 0 1 x y Tm`. All text goes through the `push_text` helper so this cannot regress. A unit test covers the operand order.
2. **`Td` is relative, not absolute.** `Td` moves relative to the start of the current line. Inside a fresh `BT` it happens to start at (0, 0), but the result is easy to misread. Use `Tm` (via `push_text`) for absolute placement.
3. **The origin is bottom-left and y grows upward.** A4 is 595 x 842 points. "Near the top" means a large y value. Leave a margin of at least ~50pt. A staff whose lines extend past y = 842 is clipped or disappears entirely.
4. **Content streams are per page, not per measure.** Decoding, appending and re-encoding a shared stream duplicated operations. Creating a new stream per measure and swapping the page's `/Contents` meant only the **last** measure on a page survived. This is fixed. `render_pages` collects all operations for a page in one `Vec<Operation>`, and `save_to_file` writes each page's stream exactly once. Never touch a page's `/Contents` after creating it. This bug also explained earlier "the staff is cut off at the top" reports. Each new measure was also placed *higher* because y grows upward, so with several measures the visible one was above the page edge.
5. **Text glyphs are not music symbols.** Helvetica `@` / `O` / `f` look nothing like noteheads or flags and don't scale with the staff. Draw music symbols as vector paths (see `push_notehead`). Use text only for numbers and letters (time signatures, labels), or for a real music font such as an embedded SMuFL font (Bravura) later.
6. **Pitch-to-staff mapping must be diatonic.** Vertical position = `(octave * 7 + diatonic_step) - clef.bottom_line_diatonic_index()`, multiplied by half a staff gap. In treble clef E4 = 0 (bottom line), B4 = 4 (middle line), F5 = 8 (top line). In bass clef G2 = 0, D3 = 4, A3 = 8. The old `(midi - 60) % 14` formula mixed chromatic and diatonic units. The vertical position now comes from the *spelled* pitch (letter and octave), not from the MIDI number, so Bb lands on the B line in flat keys. See Note Spelling and Accidentals below.

### Page and Measure Layout

Implemented by `layout`, `render_pages` and `draw_measure` in `pdf.rs`. Layout is a pure step (`layout` returns a `MeasureSlot` per measure) that is separate from drawing, so it is unit-tested without producing a PDF.

- **Page:** A4 (595 x 842pt) with 40pt side margins and 60pt top and bottom margins. The first staff sits at the top of the page, and rows go **downward**, which means decreasing y.
- **Rows ("systems"):** each row starts with a header (clef, key signature, and on the first row the time signature). The measures of a row fill the rest of the width between the margins. The first row has the widest header, so it decides how many measures of the preferred `measure_width` (200pt by default) fit in a row. That gives 2 per row on A4. Rows with a smaller header get slightly wider measures. The last row keeps the same measure width and is not stretched, as in printed music.
- **Spacing between rows:** `system_spacing` (60pt) measured between the bottom staff lines. That leaves room for ledger lines and stems. A new page starts when the next row would cross the bottom margin (12 rows per page by default).
- **Staff lines** are drawn once per row, by that row's first measure, so they run unbroken through the barlines.
- **Barline:** every measure ends with a thin barline. There is no barline at the start of a row, as on a single-staff part. There is no final double barline yet.
- **Note spacing** inside a measure divides the available width evenly by note *count*. It does not scale with duration, so a whole note takes as much room as a quarter note. Duration-proportional spacing is future work.
- **Empty input** produces one blank page rather than an invalid PDF.

### Bass Clef

The clef is a setting of the renderer (`PdfNotationRenderer::with_clef`) and of the command line (`--clef bass`, short `-c`). It is **not** stored in a measure, so one exercise has one clef. Notes keep their pitch, and only the staff changes how they are written.

**What changes with the clef** (all driven by `domain::Clef`, so there is one place to look):
- **Pitch to staff mapping:** the bass staff's lines are **G B D F A** (G2 on the bottom line, D3 in the middle, A3 on top) and its spaces are **A C E G**. In steps above the bottom line: G2 = 0, A2 = 1, B2 = 2, C3 = 3, D3 = 4, F3 = 6, A3 = 8, B3 = 9. **Middle C is on the first ledger line above the staff** (position 10) and F2 is in the space below it (-1). The same written position is a different note in each clef: the bottom line is E4 in treble clef and G2 in bass clef.
- **Clef glyph:** `BASS_CLEF` (Bravura `fClef`), origin on the F line, drawn at staff position 6.
- **Key signature positions:** each accidental is the treble position **two steps (a third) lower** (`BASS_SHARP_POSITIONS`, `BASS_FLAT_POSITIONS`). Sharps F C G D A E B are at 6, 3, 7, 4, 1, 5, 2 (F sharp on the fourth line, B sharp on the second line). Flats B E A D G C F are at 2, 5, 1, 4, 0, 3, **-1**. The seventh flat, F flat, hangs in the space **just below the staff**, which is where printed bass clef key signatures put it. It needs no ledger line, like the G sharp above the treble staff. Source: the guitarland.com key signature pages, and Dorico's rule that the pattern is the same in every clef except for tenor sharps.
- **Everything else is the same:** stems, flags, beams, ledger lines, accidentals, dots and time signatures work from staff *positions*, which already assume a five-line staff with the middle line at position 4. They needed no change.

**Default range** (`octave_midi_range(count, clef)` in `main.rs`): the whole-octave range, starting on a C, whose centre is closest to the middle line of the staff. Two octaves (the default) are **C4 to B5** in treble clef and **C2 to B3** in bass clef. One octave is C4 to B4 and C3 to B3. Larger spans grow around that. Treble results are the same as before this function took a clef. The melody is also pulled toward the middle of the range, so most notes land on the staff and a few go onto ledger lines.

**Not handled:** changing clef in the middle of a piece, a grand staff (treble and bass together), and choosing the clef from the notes automatically.

### Clef, Key Signature and Time Signature

Implemented by `header_layout`, `draw_header`, `key_signature_glyphs`, `time_signature_width` and `push_glyph` in `pdf.rs`. The shapes come from `music_glyphs.rs` (see below). The layout is a pure function, `header_layout`, that returns the x position of each symbol, and it is unit-tested.

**Rules** (sources: Gould's *Behind Bars* pp. 42 to 43, Dorico and Open Music Theory):
- **Order:** clef, then key signature, then time signature, left to right. The clef and key signature are repeated at the start of **every row**. The time signature is shown only at the start of the piece (the first row).
- **Spacing:** 1 to 1.5 staff spaces between the clef, key signature and time signature. We use `SYMBOL_GAP_SPACES` = 1.25. The largest gap should be before the first note, with 2 to 3 spaces to a note without an accidental. Our measures start with a 3-space padding to the notehead center.
- **Clefs:** the treble clef's curl wraps the **G line** (second line from the bottom), so its glyph origin is at staff position 2. The bass clef's two dots straddle the **F line** (fourth line from the bottom), so its origin is at staff position 6. `Clef::reference_line_steps` has both. See Bass Clef below.
- **Key signature order and position (treble):** sharps F C G D A E B at positions 8, 5, 9, 6, 3, 7, 4 (first sharp on the top line). Flats B E A D G C F at positions 4, 7, 3, 6, 2, 5, 1 (first flat on the middle line). Each accidental is in one fixed octave, so none needs a ledger line. The G sharp sits in the space just above the staff. A C major key signature draws nothing.
- **Time signature:** each digit is 2 staff spaces tall, so numerator and denominator together fill the staff. The numerator is centered on position 6 and the denominator on position 2. Multi-digit numbers are laid side by side. The numerator and denominator are centered on each other.
- **Gap estimate:** `KEY_ACCIDENTAL_GAP_SPACES` (0.2) and `HEADER_LEFT_PAD_SPACES` (1.0) are my estimates. The sources give no figures. One non-authoritative site claimed 0.25.
- **Common time** (the C symbol) is not implemented. 4/4 is drawn as numerals.
- **Only treble and bass clef.** Alto, tenor and percussion clefs are not implemented. Adding one needs its glyph, its reference line, its bottom line on the diatonic scale and its key signature positions (the tenor clef is the exception that breaks the pattern for sharps).

**Where the shapes come from, and the license:**
- The clef, sharp, flat and digit outlines in `src/presentation/music_glyphs.rs` are **generated** from the **Bravura** music font by `tools/extract_music_glyphs.py`. Do not edit the generated file. Rerun the script instead, using `Bravura.otf` from `https://github.com/steinbergmedia/bravura` (`redist/otf/Bravura.otf`) and `pip install fonttools`.
- Bravura is Copyright (c) 2015 Steinberg Media Technologies GmbH, under the **SIL Open Font License 1.1**. The license is in `LICENSES/Bravura-OFL.txt`, and the generated file carries the copyright notice. The OFL permits embedding and derivative use but requires that notice and license to travel with the outlines. The Reserved Font Name "Bravura" must not be used for a modified font. Keep both when moving or redistributing this code.
- Coordinates are in **staff spaces**, y up, following SMuFL: the clef origin is the G line, accidentals are centered on their staff position, and digits are centered vertically on their origin. `push_glyph` scales spaces to points with one `cm` transform and fills with `f` (non-zero winding, which the outlines' counters rely on).
- To add a glyph (natural sign, other clefs, rests), add it to the `GLYPHS` list in the script and regenerate.

### Note Spelling and Accidentals

A MIDI number does not say whether a black key is a sharp or a flat, so the renderer spells each pitch before drawing. The logic is in `src/presentation/spelling.rs` (pure, unit-tested, no PDF code), and `pdf.rs` only draws the result. `spell_measure` takes the MIDI notes of one measure and the key signature, and returns each note's letter, alteration, octave and optional accidental sign.

**Spelling rules** (sources: Open Music Theory, Tomus, the Dorico accidental duration rule, Gould p. 78):
1. A pitch in the key's scale is spelled the way the key signature says: C# in D major, Bb in Bb major, E# in C# major, Cb in Cb major. Such notes need **no sign**.
2. Another white key is spelled as the natural note. If the key signature alters that letter, it gets a natural sign (F natural in G major).
3. A remaining black key is a sharp in sharp keys and a flat in flat keys. In a key with no accidentals, the textbook convention applies: **sharps when the melody rises, flats when it falls**. The renderer knows the previous note of the measure. The first note of a measure counts as rising.
4. The written octave follows the letter, not the sound: B#3 sounds in octave 4, and Cb4 sounds in octave 3. A unit test checks that letter + alteration + octave always gives back the original MIDI number for 7 keys, every pitch from 24 to 108, and both directions.

**Accidental signs** (common practice duration rule):
- A sign is needed when a note's alteration differs from what currently applies to its **letter and octave**. At first that is the key signature, and afterwards the last sign written in the bar.
- A sign holds for the **rest of the bar** and for that **octave only**. It does not carry into other octaves. The barline cancels it, so every measure starts from the key signature again.
- Returning to the key's pitch after an altered note gets a sign too: a natural, or the sharp or flat that the key signature gives. For example F natural then F# in G major gives a natural sign and then a sharp sign.
- Cautionary (courtesy) accidentals are **not** drawn. Dorico makes them optional.

**Drawing:** the sign sits directly to the left of the notehead, centered on the note's line or space, whatever the stem direction. The gap to the notehead is `ACCIDENTAL_GAP_SPACES` (0.25), **my estimate**, since Gould only says it closes up as far as it can without colliding. For notes with ledger lines the sign clears the ledger lines' left ends. The glyphs (sharp, flat, natural) come from Bravura through the extraction script, like the key signature.

**Not handled:** double sharps and flats, key changes in the middle of a piece (each row's key signature comes from its first measure), minor-key spelling (a raised leading tone in a flat minor key such as D minor, where C# would be correct, is spelled as a flat), accidentals tied across barlines, and chords. The ASCII renderer does not use this spelling module yet. It still prints sharps only, for example `A#4` where a B♭ key would write Bb4.

Exercises from the generator contain only notes of the key, so they have **no accidentals**. The accidental code paths are still exercised by unit tests and by hand-built measures.

### Noteheads by Duration

Chosen in `draw_note` (`pdf.rs`). Getting these wrong makes durations unreadable, so check all three in any render you do.

| Duration | Head | Stem | Flags or beams |
|---|---|---|---|
| Whole | wide hollow ring, **no tilt**, tilted hole (`WHOLE_NOTEHEAD`) | **none** | none |
| Half | hollow ring, same size as a quarter head (`HALF_NOTEHEAD`, scaled) | **yes** | none |
| Quarter and shorter | solid tilted oval (`push_notehead`) | yes | eighths and shorter |

- **Half notes need a stem.** An earlier version drew stems only for quarter notes and shorter, so a half note was just a hollow circle and looked exactly like a whole note.
- **The whole note head is wider** than the others: 1.688 staff spaces against about 1.26 for the oval, and exactly one space tall. Everything that sits beside the head follows its real width: ledger lines, accidentals and dots take a `half_width` argument, from `notehead_half_width(duration)`. Use that function, not `notehead_stem_anchor`, for anything that is not a stem.
- **The half head is Bravura's `noteheadHalf`, stretched to the exact bounding box of the filled head** (`push_half_notehead`: the width of `notehead_stem_anchor`, the height of `notehead_half_height`). That keeps stems, dots and accidentals valid for both heads, and gives the same heavy ring as the whole note. The glyph is one space tall and 1.18 wide, so the vertical scale assumes a unit height. A test checks that.
- Both hollow heads and every other music symbol come from `music_glyphs.rs`. Regenerate it with `tools/extract_music_glyphs.py` if you add a head.

### Engraving Rules for Stems

Sources: MPA *Standard Music Notation Practice*, Elaine Gould's *Behind Bars*, and the SMuFL specification (Bravura metadata). These are implemented by `stem_geometry` / `push_stem` in `pdf.rs`.

- **Which notes:** every note shorter than a whole note has a stem (half notes included).
- **Direction:** notes on the middle line or higher are stemmed **down**. Notes below the middle line are stemmed **up**. (Middle-line notes only go up when beaming or voices call for it. Beamed groups decide direction together, see below.)
- **Attachment:** an up-stem attaches to the **right** edge of the notehead. A down-stem attaches to the **left** edge. Never draw a stem through the notehead's center.
- **Length:** 3.5 staff *spaces* (one octave) measured from the notehead center. A staff space is `STAFF_LINE_GAP`, **not** `STAFF_STEP`, which is half a space. Using the wrong unit made the first version half as long as it should be.
- **Ledger lines:** if a standard-length stem would not reach the middle staff line, extend it to the middle line.
- **Thickness:** about 0.12 staff spaces (Bravura's `stemThickness`).
- **Anchor point:** Bravura starts an up-stem 0.184 spaces above the notehead center and a down-stem 0.184 below it. With a tilted ellipse, use the point where the ellipse is widest (`notehead_stem_anchor`). Inset the line by half its width so the stroke's outer edge lines up with the notehead edge.
- Flags are covered in the next section.
- Sources do not agree on beamed groups, chords, or multiple voices. Handle these separately when they come up.

### Engraving Rules for Beams

Sources: Gould's *Behind Bars* (Beams, pp. 17 to 23, and Beaming according to the metre, p. 153), Ted Ross as quoted in Dorico's blog and forum, and Dorico's beam slant documentation. All of the logic is in `src/presentation/beaming.rs`, which is pure and uses staff spaces and integer ticks. `pdf.rs` only converts to points and draws (`beamed_notes`, `push_beam`).

**Which notes are beamed together** (`beam_groups`):
- Notes shorter than a quarter that start in the same **beat** are beamed together, in every metre. The beat is the denominator note, or three of them in compound time (3/8, 6/8, 9/8, 12/8): `beat_ticks`.
- Beams **never cross a barline** (each measure is grouped on its own) and never cross the middle of a 4/4 bar.
- A run of **eighths only** may continue across beats within half a bar in 4/4 (beats 1 and 2, 3 and 4) and across the whole of a 2/4 bar. Anything shorter than an eighth in the run keeps the beats apart. In 3/4 each beat is beamed on its own.
- A note alone in its beat keeps its flag. A group needs at least two notes.
- Time is counted in integer **ticks** (96 per quarter note) so dotted notes and triplets never suffer from float error: `duration_ticks`, `note_onsets`.

**Stem direction** (`group_stems_up`): the majority of notes above or below the middle line decides. If they are level, the note furthest from the middle line decides, and if that ties, the stems go **down**. All stems in a group point the same way. Gould and Ross also say a *minority* of very distant notes may win. That exception is not implemented.

**Beam geometry** (`layout_group`, in staff spaces, bottom line = 0):
- A beam is **0.5 space thick** and the gap between beams is **0.25 space** (`BEAM_THICKNESS`, `BEAM_GAP`). A beam is a parallelogram with vertical edges, so it keeps the same thickness at every slant.
- The **outer beam's end** must **sit on**, **hang from**, or be **centred on** a staff line, so that no beam begins or ends in the middle of a space (Ross's sit, hang or straddle). In stem coordinates the allowed values are every whole space and every whole space plus 0.25. The search tries every pair of allowed values for the two ends and picks the cheapest.
- **Stem length** from the notehead centre to the outer edge of the beam: at least **3.25** (the standard 3.5 for a note in a space and 3.25 on a line, as Gould says), and long enough that the innermost beam's centre is at least 2.25 from the notehead, so 3 beams need 4.0 and 4 beams need 4.75 (`min_stem_length`). Stems of far-away notes are extended to reach the middle line.
- **Slant** follows the interval between the first and last note: 0.25 space per step, at most 1.0 space (1.25 for groups of four or more, 0.5 when notes are closer than 3 spaces, as Gould says). The beam is **flat** when the first and last notes are the same, when an inner note is closer to the beam than both outer notes (a concave group), or when most notes move against the outer interval. Because the ends must snap, the real slant can differ from the wanted one by a quarter space or more.
- **Secondary beams** (one per further beam level) are the full width of the group where consecutive notes have that many beams. A single note with extra beams gets a **stub**, a notehead long. It points to the other half of its parent beat unit: first half points right, second half left (`stub_points_right`), so a sixteenth after a dotted eighth points back at it. At the ends of a group the stub flips inward so it never sticks out.
- Stems end on the **outer** edge of the beam and run through all inner beams.

**Not handled** (all deliberate for now):
- No rests, so beams over rests and stemlets are not needed. The generator makes none.
- Widening the gap between beams so slants smaller than a whole space still snap (Gould's and Ross's compromise for 3 or more beams). Instead the slant is rounded to what the snapping allows.
- Avoiding "wedges", the thin triangles between a stem, a staff line and a slanted beam.
- Chords, cross-staff beams, grace notes, tuplet brackets, and beams that carry a clef or key change.
- Mid-measure time signature changes (each measure uses its own time signature).
- **Tests** run a sweep of 800+ group shapes to check that every stem is long enough, lies on the beam, and that no slant exceeds the limit. Rendering tests build measures by hand.

### Engraving Rules for Flags

Sources: Gould's *Behind Bars* (tails, p. 15 to 16) and the SMuFL glyph registration notes. Implemented by `flag_count` (in `beaming.rs`), `FLAG_OUTLINE` and `push_flags` in `pdf.rs`. Flags are drawn only on notes that are **not** beamed. A note that is alone in its beat keeps its flag (see Engraving Rules for Beams).

- **Count:** eighth 1, sixteenth 2, thirty-second 3, sixty-fourth 4.
- **Side:** always to the **right** of the stem, for up-stems and down-stems alike. Flags hang from the stem's free end and point toward the notehead. The flag is flush with the stem's left edge (SMuFL registers flag glyphs at x = 0 on the left of the stem, with y = 0 at the stem end).
- **Length:** an eighth-note tail is 2.5 to 3.25 spaces long (3 to 3.25 is the norm). On an up-stem it ends opposite or just above the notehead. A down-stem tail may curve as far as touching the notehead. Don't let a tail overshoot the notehead.
- **Several flags:** each further flag sits a little less than a space from the previous one, on the notehead side. The combined stack is 1/4 to 1/2 space longer than a single tail. So later flags are slightly shorter than the first (`FLAG_SPACING_SPACES`, `FLAG_EXTRA_DEPTH_SPACES`).
- **Stem length:** the standard 3.5-space stem fits one or two flags. For each flag beyond two, lengthen the stem by one flag spacing. This is handled inside `stem_geometry` through its `flags` argument. Compute the stem **once** and pass the same `StemGeometry` to both `push_stem` and `push_flags`.
- **Shape:** a filled, tapered curve, thick at the stem and pointed at the tip. It is defined in unit coordinates (`FLAG_OUTLINE`) and scaled by flag width and length, so the same outline serves every flag count and both directions (down-stems just flip the y direction).
- **Tuning the outline** (it took three tries): too much thickness merges 2 or more flags into a solid blob, and too little makes hairline arcs that don't read as flags. A root thickness of about 0.5 space and a mid-curve thickness of about 0.4 space works at 0.8 space spacing. Check renders at 4x zoom or more, with every count and both directions, before trusting a change.
- **Open points:** the 0.8-space flag spacing and the extra-depth value are my reading of Gould's "a little less than a space" and "1/4 to 1/2 space longer". Gould says editions differ. SMuFL has shorter flag alternates to avoid dot collisions, which we don't use (see the dot section).
- **Earlier mistake, to avoid repeating:** the first flag attempt recomputed stem direction inside the flag function from the note's own y. That made it always false, so every flag was drawn below the note. Pass the already-computed direction in.

### Engraving Rules for Augmentation Dots

Sources: Gould's *Behind Bars* (quoted in the LilyPond mailing list), the Finale documentation, and the Bravura FONTLOG. Implemented by `dot_position`, `push_dot` and `flag_right_extent` in `pdf.rs`. Only single dots are supported, because `Note` has a `dotted` flag and no dot count.

- **Never on a line:** a dot sits in a space, never on a staff line or a ledger line. A note **on a line** gets its dot in the space **above**. A note **in a space** gets its dot level with the note. Line or space is decided by the staff position: even positions are lines, odd are spaces. This also holds for ledger lines, so use `rem_euclid(2)` and not `% 2`, which breaks for negative positions below the staff.
- **Position:** to the right of the notehead and clear of the stem. An up-stem sits on the notehead's right edge, so the dot starts a gap beyond it.
- **Up-stem flags:** on an up-stem note with flags, the dot moves to the **right of the flags**, because the flags hang right next to the notehead and would hit it. Finale does the same by default. SMuFL's alternative is a shorter flag glyph. Down-stems need no adjustment, since their flags are at the far end of the stem. `flag_right_extent` samples the flag curves to find where they actually end. The control points overshoot the curve.
- **Size:** 0.4 staff spaces across (Bravura 1.03 changed it from 0.5 to 2/5).
- **Gap:** 0.5 staff space between the notehead (or flags) and the dot. **This is my estimate.** The sources consulted gave no figure. Compare against a printed score and adjust `DOT_GAP_SPACES` if it looks off.
- **Not handled:** double dots, dots on chords (each notehead takes a dot, vertically aligned, one per space), notes in two voices.
- **Remember:** when a measure holds dotted notes, they need slightly more horizontal room than undotted ones. Spacing currently only counts notes.

### Engraving Rules for Ledger Lines

Sources: Elaine Gould's *Behind Bars* and the SMuFL `engravingDefaults`. They agree with each other. A site called ScoreLabs claims 0.5 to 0.75 space of protrusion and staff-line thickness, which contradicts both, so it was not used. Implemented by `ledger_line_positions`, `ledger_line_extent` and `push_ledger_lines` in `pdf.rs`.

- **Spacing:** the same as the staff lines, so they are an extension of the staff.
- **Which lines:** only the lines between the staff and the note, plus the note's own line if it sits on one. A note in the first space above or below the staff needs **none**. In steps above the bottom line (line positions are even): below the staff draw every even `n` with `position <= n <= -2`. Above the staff draw every even `n` with `10 <= n <= position`.
- **Length:** the notehead plus an extension on each side, centered on the notehead. SMuFL's extension is 0.4 staff spaces. With our notehead that gives about 2.06 spaces in total, matching Gould's "just over two spaces".
- **Thickness:** visibly thicker than staff lines. Gould says about twice as thick. Staff lines are 0.13 spaces (Bravura) and ledger lines 0.26. The staff line width has to be set explicitly with `w`, because PDF's default of 1pt (0.2 spaces) is thicker than Bravura's ledger lines.
- **Draw order:** ledger lines first, then the notehead, then the stem. Hollow noteheads must not be filled with white, because the line should show through the head.
- **Adjacent notes:** ledger lines of neighboring notes should not join up. This is **not handled yet**. It is only a problem when a measure has more than about 13 notes in the current width, because each note gets `staff_width` divided by the note count. Shorten the lines when that happens.
- **Stems on ledger-line notes** are extended to the middle line (see the stem rules above).
- Chords, accidentals next to ledger lines and displaced adjacent notes need special handling in Gould. They are out of scope for now.

### General Advice for Drawing PDF Symbols

- **Add one symbol type at a time** and look at the rendered result before adding the next. Inspecting operators with `strings file.pdf` is useful, but it does not tell you whether the output *looks* right.
- **Preview without a GUI on macOS:** `sips -s format png -Z 2400 file.pdf --out file.png` renders the first page to an image you can inspect. Crop with `sips -c <h> <w> --cropOffset <y> <x>`.
- **Keep geometry separate from drawing.** Compute positions in a pure function that returns a struct (like `StemGeometry`) and unit-test that. The function that pushes PDF operators then only translates it.
- **Convert units deliberately.** Use staff *spaces* (`STAFF_LINE_GAP`) for sizes and staff *steps* (`STAFF_STEP`, half a space) for pitch positions. Mixing them up gives results that are off by a factor of two and still look plausible.
- **Check the rendered image against the rules, not just that something drew.** Open the rendered image and compare it with the engraving rules. Passing tests and operators in the PDF are not enough.
- **Set line widths explicitly** inside a `q` ... `Q` block for everything you draw (staff, ledger, stems). Don't rely on PDF's default 1pt, because relative thicknesses (ledger lines vs staff lines) can then come out backwards.
- **Don't add symbols that weren't asked for.** Add one symbol type at a time, as the first bullet says.
- **Write every symbol as a small free function** (`push_text`, `push_notehead`, ...) that takes `&mut Vec<Operation>` and absolute coordinates. Unit-test the operators it emits (order, balance, closing ops).
- **Wrap transformed drawing in `q` ... `Q`.** Use `cm` (`cos sin -sin cos x y cm`) to translate/rotate a symbol drawn around its own origin (0, 0). This keeps symbol geometry simple, and the graphics-state change doesn't leak into later drawing (line width, color, transforms).
- **Always end a path with a painting operator.** `S` strokes, `f` fills, `B` fills and strokes. `h` closes a subpath but does not paint it. A path left without a painting operator gets combined with the next one, which produces stray lines.
- **Text must sit inside `BT` ... `ET`**, with `Tf` (font name as `Object::Name`, plus size) set before `Tj`. The font name must match the page's `/Resources /Font` dictionary (`F1` = Helvetica).
- **Size symbols from the staff, not from fonts.** Derive every dimension from `STAFF_LINE_GAP` (notehead radii, stem length ≈ 3.5 gaps, ledger line width ≈ 2.5 notehead widths). Changing the staff size then scales everything consistently.
- **Use named constants for all geometry** (see the top of `pdf.rs`). This follows the "avoid magic numbers" rule below.
- **Ellipses and curves:** approximate each quarter of an ellipse with one cubic Bezier (`c`), with control points at `radius * 0.5523` (`BEZIER_CIRCLE_KAPPA`).
- **Filled vs hollow:** a solid shape uses `f`. A ring (a hollow notehead) is one filled outline made of two contours, the outer edge and the hole, which the non-zero fill turns into a ring. Do not stroke a hollow head with `S`: it comes out too light next to the real heads. Stems, barlines and staff lines are strokes and need an explicit `w` line width set inside a `q`/`Q` block.

## Guidelines for Future Agents

### Before Making Changes

1. **Read this file first** to understand the architecture
2. **Add new domain types** to `src/domain/` if adding new concepts
3. **Add generation logic** to `src/generator/` 
4. **Add presentation** to `src/presentation/` with the `NotationRenderer` trait
5. **Update tests** when changing domain types

### Code Style

1. **Prefer Rust idioms**: Use `Option`, `Result`, iterators, pattern matching
2. **Add doc comments** for public types and functions
3. **Add unit tests** for all new types and logic
4. **Keep functions small**: Single responsibility principle
5. **Avoid magic numbers**: Use named constants for music theory values

### Testing

1. **Domain types**: Test in isolation, no mocks needed
2. **Generator**: Test with known inputs, verify outputs are valid
3. **Presentation**: Test rendering produces expected output

### Extension Points

The following modules are designed for easy extension:

- **New duration types**: Add to `NoteDuration` enum in `domain/note.rs`
- **New interval types**: Add to `Interval` enum in `domain/interval.rs`
- **New key signatures**: Update `KeySignature::calculate_signature` in `domain/key.rs`
- **New renderers**: Implement `NotationRenderer` trait in `presentation/`
- **New generation strategies**: Implement new functions in `generator/rules.rs`

### Breaking Changes

If you need to change the domain model:

1. **Update all tests** first
2. **Update all generators** that consume the changed types
3. **Update all renderers** that display the changed types
4. **Update CLI parsing** if command-line arguments change
5. **Update README.md** if user-facing behavior changes

## Resources

- MIDI note number reference: https://www.instrumentslab.com/midi-tables/midi-note-numbers/
- Music theory reference: TonalHARM library concepts
- Circle of fifths: Standard music theory
