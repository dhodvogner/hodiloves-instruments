# The music theory inside Chordwave

Chordwave is only useful if its suggestions are *right* and *explainable*. This is what is implemented, why,
and where the approximations are. Everything lives in plain, tested functions
(`src/theory.rs`, `src/suggest.rs`, `src/generate.rs`, `src/style.rs`).

## 1. Scales and the chord palette

A key plus a scale (major, natural / harmonic / melodic minor, Dorian, Phrygian, Lydian, Mixolydian,
Locrian, Phrygian dominant) defines seven notes. A **diatonic chord** is built by *stacking thirds inside the
scale*: take a degree, skip one scale note, take the next, and so on. Nothing is hard-coded per mode, so the
palette is correct for every scale, including the unusual ones:

| Scale (A) | i | ii | III | iv | v | VI | VII |
|---|---|---|---|---|---|---|---|
| Natural minor, triads | Am | Bdim | C | Dm | Em | F | G |
| Natural minor, 7ths | Am7 | Bm7b5 | Cmaj7 | Dm7 | Em7 | Fmaj7 | G7 |
| Harmonic minor, 7ths | Am(maj7) | Bm7b5 | Cmaj7#5 | Dm7 | **E7** | Fmaj7 | G#dim7 |

Why this matters: in harmonic minor the raised 7th makes a *major* V (E7) whose G# pulls back to A. That
"dominant" pull is the strongest tension-and-release device in tonal music, and it is why Dark Jungle or Night
Drive style loops use it.

Extensions (7th, 9th, 11th, sus2, sus4, add9, 6th) are derived the same way, and fall back to the simpler chord
when the stack has no common name (for example a 9th that would be a b9 over a minor chord).

The colour coding follows the common convention: major-type chords green, minor-type purple, everything else
(sus, diminished, augmented, half-diminished) grey. A **filled dot** means every note of the chord is in the
home scale; a **hollow dot** means the chord *borrows* from outside it.

## 2. Roman numerals and harmonic function

Chords are labelled by their scale degree (`i`, `VI`, `VII7`, `iiø7`, `bII`). Upper case is major-type, lower case
minor-type or diminished. Roman numerals describe the *relationship* to the key, so a progression such as
**i - VI - III - VII** is the same idea in every key.

Each diatonic chord also has a **function**:

- **Tonic** (I, iii, vi, and III in minor): rest, "home".
- **Predominant** (ii, IV, and VI in minor): leaves home, builds anticipation.
- **Dominant** (V, vii): tension that wants to resolve to the tonic.
- **Colour**: a chord outside the scale.

The usual flow is Tonic -> Predominant -> Dominant -> Tonic. This is a simplification (loop-based genres
often avoid the dominant on purpose, which is exactly why synthwave loops feel like they keep driving), so
function is *used as a hint, never as a rule*.

## 3. "What comes next": how a suggestion is scored

For every candidate chord after the current one, the score combines:

1. **Root motion** between the two chords. Falling a fifth (equivalently, rising a fourth) is the strongest and
   most common motion in tonal music (V-I, ii-V, vi-ii). Steps and thirds are weaker but smooth; a tritone leap
   is the most surprising.
2. **Function flow** (Dominant -> Tonic is the best, Predominant -> Dominant next, and so on).
3. **Common tones**: chords that share notes sound connected; this also predicts how smooth the voice leading is.
4. **Genre habits**: a small table of typical moves per genre and mode (for example synthwave minor:
   i-VI, VI-III, III-VII, VII-i, i-VII; drum and bass: i-VII, VII-VI, i-VI; pop major: I-V, V-vi, vi-IV, IV-I).
5. **Borrowed chords** get a penalty and are labelled with the mode they come from ("borrowed from Harmonic
   Minor: a colour outside the key"), so you can choose them knowingly.

The text next to each suggestion is generated from the same facts that produced the score ("the root falls a
fifth: the strongest, most natural motion in tonal music; 1 note in common; resolves tension back home"), so
the panel explains itself and does not just rank.

## 4. Matching chords to a melody

For the melody notes (weighted by length and velocity), each candidate chord is scored by:

- how much of the melody it **contains** (chord tones),
- a penalty for melody notes a **half step above a chord tone** (a minor-9th grind),
- a bonus for staying **inside the key** and for the chord root being present,
- a small preference for simpler chords and for the genre's typical qualities.

Notes that are not chord tones are reported as *non-chord tones* (passing notes), because a good melody has
them. The same machinery powers **Re-harmonise** (best chord per existing chord slot) and **Find chords**
(one chord per bar of an imported track, identical neighbours merged).

## 5. Key detection

The **Krumhansl-Schmuckler** algorithm: correlate the (length and velocity weighted) pitch-class histogram of the
notes with the perceptual major and minor key profiles of Krumhansl and Kessler, rotated to all 12 tonics, and
take the best correlation. It is reliable for melodies that lean on the tonic and fifth. Relative keys
(A minor / C major) share all notes, so the suggestion can land on the relative key: check it by ear.

## 6. Borrowing chords: adjacent keys and parallel modes

- **Adjacent keys** step around the **circle of fifths**. A key one step away shares six of seven notes with the
  home key, so its chords are "almost" in key (subtle colour). Further steps add more foreign notes (more
  exotic).
- **Parallel modes** keep the tonic and change the scale (A minor -> A Dorian / A major / A Phrygian...).
  Borrowing a chord from a parallel mode is called **modal interchange**: the iv in a major key, the major IV
  in Dorian, the bII in Phrygian. It is how pop and film music add colour without leaving the key.

## 7. Voicing and voice leading

A chord is stored as *what it is* (root, quality, optional slash bass) and turned into notes late:

- **Inversion** rotates the lowest notes up an octave. **Open voicing** drops the second-highest note by an
  octave (drop-2), which spreads the chord out.
- **Auto voicing** (the default) chooses, for each chord, the inversion and octave that moves the *least* from
  the previous chord. Smooth voice leading is what makes a pad progression sound professional instead of
  jumping around.
- A **slash bass** (C/E) puts another note in the bass, for stepwise bass lines.

## 8. Generated bass and melody

**Bass**: follows the chord root (or the slash bass), placed in a register that suits the genre (drum and bass
sits an octave lower than synthwave) with rhythm patterns: sustained, driving 8ths and octave bounce
(synthwave), offbeat roots (house / trance), the two-step and a rolling pattern (drum and bass).

**Melody**: a deliberately simple, rules-based hook writer:

- Notes on **strong beats are chord tones** (the notes that are "safe" over the chord).
- Notes in between move by **step through the scale** (passing notes), with the occasional skip.
- Two **rhythms alternate bar by bar** (question and answer) and a **motif** of stepwise moves is reused, with
  variation, so it sounds intentional.
- The **contour** rises in the first half and falls in the second.
- A third repeat of the same pitch is avoided, each chord **ends on a chord tone** and the last note resolves to
  the root.
- *Density* changes how busy the rhythm is; the genre picks rhythm pools (long notes for cinematic, off-grid
  16ths for drum and bass, steady 8ths for trance).

It is a starting point to edit in the piano roll, not a finished tune. Seeds are deterministic, so
**Generate** gives a different melody every press but the same sequence of melodies every session.

## 9. Genre notes (synthwave and drum and bass first)

- **Synthwave** lives in natural minor (and Dorian / Lydian for the dreamy ones). The signature is a four-chord
  loop (i-VI-III-VII, i-VII-VI-VII), often with 7ths or add9 for the polysynth glow, and gated or arpeggiated
  chords over driving 8th-note bass.
- **Drum and bass** harmony is usually *slow* (a chord per two bars) so the drums and bass carry the energy:
  minor vamps (i-VII-VI-VII), two-chord loops (im7-VImaj7), Phrygian bII for neurofunk menace, and 9th chords
  for liquid. The bass is the star, hence the two-step and rolling patterns and the low bass range.
- Other genres are available as starting points (pop, cinematic, lo-fi / jazz, trance / house), and the genre
  only biases suggestions and defaults: any chord, scale or pattern can be chosen in any genre.

## 10. What is not modelled

- Only 4/4 time. No tempo changes, no swing, no humanisation (everything is on the grid).
- Chord detection is pitch-class based: it cannot hear bass inversions or distinguish a very dense pad from
  a cluster.
- Secondary dominants (V/V, V/vi), tritone substitutions and chromatic mediants are not suggested on their own,
  although you can put any of those chords in the sequence from the palette (parallel modes / adjacent keys)
  or by editing.

## Further reading

- Krumhansl, *Cognitive Foundations of Musical Pitch* (1990): the key profiles used for key detection.
- Piston, *Harmony* and Aldwell and Schachter, *Harmony and Voice Leading*: function, root motion and
  voice leading.
- Tymoczko, *A Geometry of Music* (2011): why smooth voice leading and common tones make chords sound related.
- Berklee's modal interchange and chord-scale material for borrowed chords.
