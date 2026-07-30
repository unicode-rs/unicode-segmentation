// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::tables::grapheme::{self as gr, GraphemeCat};
use core::cmp;

/// External iterator for grapheme clusters and byte offsets.
///
/// This struct is created by the [`grapheme_indices`] method on the [`UnicodeSegmentation`]
/// trait. See its documentation for more.
///
/// [`grapheme_indices`]: trait.UnicodeSegmentation.html#tymethod.grapheme_indices
/// [`UnicodeSegmentation`]: trait.UnicodeSegmentation.html
#[derive(Debug, Clone)]
pub struct GraphemeIndices<'a> {
    start_offset: usize,
    iter: Graphemes<'a>,
}

impl<'a> GraphemeIndices<'a> {
    #[inline]
    /// View the underlying data (the part yet to be iterated) as a slice of the original string.
    ///
    /// ```rust
    /// # use unicode_segmentation::UnicodeSegmentation;
    /// let mut iter = "abc".grapheme_indices(true);
    /// assert_eq!(iter.as_str(), "abc");
    /// iter.next();
    /// assert_eq!(iter.as_str(), "bc");
    /// iter.next();
    /// iter.next();
    /// assert_eq!(iter.as_str(), "");
    /// ```
    pub fn as_str(&self) -> &'a str {
        self.iter.as_str()
    }
}

impl<'a> Iterator for GraphemeIndices<'a> {
    type Item = (usize, &'a str);

    #[inline]
    fn next(&mut self) -> Option<(usize, &'a str)> {
        self.iter
            .next()
            .map(|s| (s.as_ptr() as usize - self.start_offset, s))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> DoubleEndedIterator for GraphemeIndices<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<(usize, &'a str)> {
        self.iter
            .next_back()
            .map(|s| (s.as_ptr() as usize - self.start_offset, s))
    }
}

/// External iterator for a string's
/// [grapheme clusters](http://www.unicode.org/reports/tr29/#Grapheme_Cluster_Boundaries).
///
/// This struct is created by the [`graphemes`] method on the [`UnicodeSegmentation`] trait. See its
/// documentation for more.
///
/// [`graphemes`]: trait.UnicodeSegmentation.html#tymethod.graphemes
/// [`UnicodeSegmentation`]: trait.UnicodeSegmentation.html
#[derive(Clone, Debug)]
pub struct Graphemes<'a> {
    string: &'a str,
    cursor: GraphemeFwd<'a>,
    cursor_back: GraphemeCursor,
}

impl<'a> Graphemes<'a> {
    #[inline]
    /// View the underlying data (the part yet to be iterated) as a slice of the original string.
    ///
    /// ```rust
    /// # use unicode_segmentation::UnicodeSegmentation;
    /// let mut iter = "abc".graphemes(true);
    /// assert_eq!(iter.as_str(), "abc");
    /// iter.next();
    /// assert_eq!(iter.as_str(), "bc");
    /// iter.next();
    /// iter.next();
    /// assert_eq!(iter.as_str(), "");
    /// ```
    pub fn as_str(&self) -> &'a str {
        &self.string[self.cursor.offset..self.cursor_back.cur_cursor()]
    }
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = &'a str;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let slen = self.cursor_back.cur_cursor() - self.cursor.offset;
        (cmp::min(slen, 1), Some(slen))
    }

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        let start = self.cursor.offset;
        if start == self.cursor_back.cur_cursor() {
            return None;
        }
        // Where a boundary lies is a property of the position,
        // so the forward driver and the backward cursor agree wherever they meet.
        let next = self.cursor.next_boundary();
        Some(&self.string[start..next])
    }
}

impl<'a> DoubleEndedIterator for Graphemes<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        let end = self.cursor_back.cur_cursor();
        if end == self.cursor.offset {
            return None;
        }
        let prev = self
            .cursor_back
            .prev_boundary(self.string, 0)
            .unwrap()
            .unwrap();
        Some(&self.string[prev..end])
    }
}

#[inline]
pub fn new_graphemes(s: &str, is_extended: bool) -> Graphemes<'_> {
    let len = s.len();
    Graphemes {
        string: s,
        cursor: GraphemeFwd::new(s, is_extended),
        cursor_back: GraphemeCursor::new(len, len, is_extended),
    }
}

#[inline]
pub fn new_grapheme_indices(s: &str, is_extended: bool) -> GraphemeIndices<'_> {
    GraphemeIndices {
        start_offset: s.as_ptr() as usize,
        iter: new_graphemes(s, is_extended),
    }
}

/// maybe unify with PairResult?
/// An enum describing information about a potential boundary.
#[derive(PartialEq, Eq, Clone, Debug)]
enum GraphemeState {
    /// No information is known.
    Unknown,
    /// It is known to not be a boundary.
    NotBreak,
    /// It is known to be a boundary.
    Break,
    /// The codepoint after it has Indic_Conjunct_Break=Consonant,
    /// so there is a break before so a boundary if it is preceded by another
    /// InCB=Consonant follwoed by a sequence consisting of one or more InCB=Linker
    /// and zero or more InCB = Extend (in any order).
    InCbConsonant,
    /// The codepoint after is a Regional Indicator Symbol, so a boundary iff
    /// it is preceded by an even number of RIS codepoints. (GB12, GB13)
    Regional,
    /// The codepoint after is Extended_Pictographic,
    /// so whether it's a boundary depends on pre-context according to GB11.
    Emoji {
        /// Whether the ZWJ char has been seen already an only a "\p{Extended_Pictographic} Extend*"
        /// part of GB11 has to be checked
        seen_zwj: bool,
    },
}

/// Cursor-based segmenter for grapheme clusters.
///
/// This allows working with ropes and other datastructures where the string is not contiguous or
/// fully known at initialization time.
#[derive(Clone, Debug)]
pub struct GraphemeCursor {
    /// Current cursor position.
    offset: usize,
    /// Total length of the string.
    len: usize,
    /// A config flag indicating whether this cursor computes legacy or extended
    /// grapheme cluster boundaries (enables GB9a and GB9b if set).
    is_extended: bool,
    /// Information about the potential boundary at `offset`
    state: GraphemeState,
    /// Category of codepoint immediately preceding cursor, if known.
    cat_before: Option<GraphemeCat>,
    /// Category of codepoint immediately after cursor, if known.
    cat_after: Option<GraphemeCat>,
    /// If set, at least one more codepoint immediately preceding this offset
    /// is needed to resolve whether there's a boundary at `offset`.
    pre_context_offset: Option<usize>,
    /// The number of `InCB=Linker` codepoints preceding `offset`
    /// (potentially intermingled with `InCB=Extend`).
    incb_linker_count: Option<usize>,
    /// The number of RIS codepoints preceding `offset`. If `pre_context_offset`
    /// is set, then counts the number of RIS between that and `offset`, otherwise
    /// is an accurate count relative to the string.
    ris_count: Option<usize>,
    /// Packed rule state for the text preceding `offset`, when it is known.
    ///
    /// The same value `GraphemeFwd::packed_state` holds; `None` here means the cursor has been seeked
    /// or walked backwards and the backward scans have to recover the context.
    ///
    /// It is known whenever the cursor has walked forwards from the start of the string,
    /// which is the case for `Graphemes` and `GraphemeIndices`.
    ///
    /// Then every rule is decided by a mask test and no rule ever has to scan backwards.
    /// It is `None` after seeking or walking backwards, where the backward scans take over again.
    packed_state: Option<u8>,
    /// Set if a call to `prev_boundary` or `next_boundary` was suspended due
    /// to needing more input.
    resuming: bool,
}

/// An error return indicating that not enough content was available in the
/// provided chunk to satisfy the query, and that more content must be provided.
#[derive(PartialEq, Eq, Debug)]
pub enum GraphemeIncomplete {
    /// More pre-context is needed. The caller should call `provide_context`
    /// with a chunk ending at the offset given, then retry the query. This
    /// will only be returned if the `chunk_start` parameter is nonzero.
    PreContext(usize),

    /// When requesting `prev_boundary`, the cursor is moving past the beginning
    /// of the current chunk, so the chunk before that is requested. This will
    /// only be returned if the `chunk_start` parameter is nonzero.
    PrevChunk,

    /// When requesting `next_boundary`, the cursor is moving past the end of the
    /// current chunk, so the chunk after that is requested. This will only be
    /// returned if the chunk ends before the `len` parameter provided on
    /// creation of the cursor.
    NextChunk, // requesting chunk following the one given

    /// An error returned when the chunk given does not contain the cursor position.
    InvalidOffset,
}

// An enum describing the result from lookup of a pair of categories.
#[derive(PartialEq, Eq)]
enum PairResult {
    /// definitely not a break
    NotBreak,
    /// definitely a break
    Break,
    /// a break iff not in extended mode
    Extended,
    /// a break unless in extended mode and preceded by
    /// a sequence of 0 or more InCB=Extend and one or more
    /// InCB = Linker (in any order),
    /// preceded by another InCB=Consonant
    InCbConsonant,
    /// a break if preceded by an even number of RIS
    Regional,
    /// a break if preceded by emoji base and (Extend)*
    Emoji,
}

/// Whether a codepoint has `Indic_Conjunct_Break=Extend`, given its grapheme category.
///
/// `InCB=Extend` is derived as
/// `[\p{gcb=Extend} \p{gcb=ZWJ}] - \p{InCB=Linker} - \p{InCB=Consonant} - U+200C`,
/// and no `InCB=Consonant` is `gcb=Extend` or `gcb=ZWJ`,
/// so the grapheme category the caller already has plus two equality tests decide it.
///
/// That saves a binary search over a dedicated range table for every codepoint the cursor walks over.
#[inline]
fn is_incb_extend(cat: GraphemeCat, ch: char) -> bool {
    matches!(cat, GraphemeCat::GC_Extend | GraphemeCat::GC_ZWJ)
        && ch != '\u{200c}'
        && !crate::tables::is_incb_linker(ch)
}

const fn pair_rules(before: GraphemeCat, after: GraphemeCat) -> PairResult {
    use self::PairResult::*;
    use GraphemeCat::*;
    match (before, after) {
        (GC_CR, GC_LF) => NotBreak,                                 // GB3
        (GC_Control | GC_CR | GC_LF, _) => Break,                   // GB4
        (_, GC_Control | GC_CR | GC_LF) => Break,                   // GB5
        (GC_L, GC_L | GC_V | GC_LV | GC_LVT) => NotBreak,           // GB6
        (GC_LV | GC_V, GC_V | GC_T) => NotBreak,                    // GB7
        (GC_LVT | GC_T, GC_T) => NotBreak,                          // GB8
        (_, GC_Extend | GC_ZWJ) => NotBreak,                        // GB9
        (_, GC_SpacingMark) => Extended,                            // GB9a
        (GC_Prepend, _) => Extended,                                // GB9b
        (_, GC_InCB_Consonant) => InCbConsonant,                    // GB9c
        (GC_ZWJ, GC_Extended_Pictographic) => Emoji,                // GB11
        (GC_Regional_Indicator, GC_Regional_Indicator) => Regional, // GB12, GB13
        (_, _) => Break,                                            // GB999
    }
}

// Bits of the packed rule state, which summarises everything
// the boundary rules can ask about the text preceding the cursor.
//
// It is a pure function of the codepoints consumed so far,
// so it carries across cluster boundaries and needs no reset.
//
// The bits a rule can ask about are laid out so that each `PAIR_MASK` entry selects exactly one of them,
// which turns every rule into a single `state & mask == 0` test.

/// Always set, so that a mask of `ALWAYS` means "never a boundary".
const ALWAYS: u8 = 1 << 0;
/// An odd run of `Regional_Indicator` immediately precedes (GB12, GB13).
const RIS_ODD: u8 = 1 << 1;
/// The last codepoint is a ZWJ that was preceded by `Extended_Pictographic Extend*` (GB11).
const EMOJI_ZWJ: u8 = 1 << 2;
/// The `InCB=Consonant` run in progress contains an `InCB=Linker` (GB9c).
const INCB_LINKED: u8 = 1 << 3;
/// `Extended_Pictographic Extend*` immediately precedes; feeds `EMOJI_ZWJ` on a ZWJ.
const EXTPIC_RUN: u8 = 1 << 4;
/// `InCB=Consonant [InCB=Extend InCB=Linker]*` precedes.
const INCB_RUN: u8 = 1 << 5;
/// This cursor computes extended clusters, enabling GB9a, GB9b and GB9c.
const EXTENDED: u8 = 1 << 6;

const _: () = assert!(
    EXTENDED >> 1 == INCB_RUN,
    "next_state_slow opens the InCB run by shifting the mode bit",
);

/// Bits that survive consuming an `Extend`: the mode, plus the pictographic run that
/// GB11 allows `Extend*` to span.
const EXTEND_KEEP: u8 = EXTENDED | ALWAYS | EXTPIC_RUN;

/// The state at the very start of a string.
#[inline]
const fn initial_state(is_extended: bool) -> u8 {
    if is_extended {
        ALWAYS | EXTENDED
    } else {
        ALWAYS
    }
}

/// The state bit that decides `check_pair`'s verdict for a pair of categories.
///
/// A mask of `0` is an unconditional boundary and can never match,
/// while `ALWAYS` is unconditionally not a boundary.
const fn pair_mask(before: GraphemeCat, after: GraphemeCat) -> u8 {
    match pair_rules(before, after) {
        PairResult::NotBreak => ALWAYS,
        PairResult::Break => 0,
        PairResult::Extended => EXTENDED,
        PairResult::InCbConsonant => INCB_LINKED,
        PairResult::Regional => RIS_ODD,
        PairResult::Emoji => EMOJI_ZWJ,
    }
}

/// `check_pair` evaluated for every category pair,
/// indexed by `(before as usize) << 4 | after as usize`.
///
/// There is a boundary between the two codepoints exactly when the packed state and
/// the pair's mask share no bit.
const PAIR_MASK: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut before = 0;
    while before < 16 {
        let mut after = 0;
        while after < 16 {
            table[before << 4 | after] = pair_mask(
                crate::tables::grapheme::CATS[before],
                crate::tables::grapheme::CATS[after],
            );
            after += 1;
        }
        before += 1;
    }
    table
};

/// The verdict for a pair of categories, for a cursor whose pre-context is unknown.
///
/// Decodes `PAIR_MASK` rather than re-running the rules: each mask is a single bit,
/// so the mapping is total, and one indexed load beats matching on the pair.
#[inline]
fn check_pair(before: GraphemeCat, after: GraphemeCat) -> PairResult {
    match PAIR_MASK[(before as usize) << 4 | after as usize] {
        ALWAYS => PairResult::NotBreak,
        RIS_ODD => PairResult::Regional,
        EMOJI_ZWJ => PairResult::Emoji,
        INCB_LINKED => PairResult::InCbConsonant,
        EXTENDED => PairResult::Extended,
        _ => PairResult::Break,
    }
}

/// State transition for consuming an `Extend` that continues an `InCB=Consonant` run.
///
/// Split out of `next_state` because it is the only transition that has to look at
/// the codepoint rather than just its category.
#[inline]
fn extend_in_incb_run(state: u8, ch: char) -> u8 {
    if ch == '\u{200c}' {
        // ZWNJ is `InCB=None`, so it ends the conjunct sequence.
        state & EXTEND_KEEP
    } else if state & INCB_LINKED != 0 || crate::tables::is_incb_linker(ch) {
        state & EXTEND_KEEP | INCB_LINKED | INCB_RUN
    } else {
        // `InCB=Extend`: the run continues but is still unlinked.
        state & EXTEND_KEEP | INCB_RUN
    }
}

// The state machine works on raw category bytes, which is what the tables hand back and
// what indexes `PAIR_MASK`, so neither driver has to widen them to the enum.
const CAT_EXTEND: u8 = GraphemeCat::GC_Extend as u8;
const CAT_PICTOGRAPHIC: u8 = GraphemeCat::GC_Extended_Pictographic as u8;
const CAT_REGIONAL: u8 = GraphemeCat::GC_Regional_Indicator as u8;
const CAT_ZWJ: u8 = GraphemeCat::GC_ZWJ as u8;
const CAT_CONSONANT: u8 = GraphemeCat::GC_InCB_Consonant as u8;

/// The only categories that carry anything forward. Everything else resets the state.
const STATEFUL_CATS: u16 =
    1 << CAT_EXTEND | 1 << CAT_PICTOGRAPHIC | 1 << CAT_REGIONAL | 1 << CAT_ZWJ | 1 << CAT_CONSONANT;

/// The packed state after consuming `ch`, whose category is `cat`.
///
/// Screening the five stateful categories with one mask test,
/// instead of letting the `match` in `next_state_slow` dispatch every codepoint.
#[inline]
fn next_state(state: u8, cat: u8, ch: char) -> u8 {
    if STATEFUL_CATS & 1 << cat == 0 {
        // Nothing to remember: keep only the mode flag.
        state & EXTENDED | ALWAYS
    } else {
        next_state_slow(state, cat, ch)
    }
}

/// The half of `next_state` that only the five stateful categories reach.
#[inline]
fn next_state_slow(state: u8, cat: u8, ch: char) -> u8 {
    let mode = state & EXTENDED;
    match cat {
        CAT_EXTEND if state & INCB_RUN != 0 => extend_in_incb_run(state, ch),
        CAT_EXTEND => state & EXTEND_KEEP,
        CAT_PICTOGRAPHIC => mode | ALWAYS | EXTPIC_RUN,
        // Flip the parity of the run of regional indicators.
        CAT_REGIONAL => mode | ((state & RIS_ODD) ^ (ALWAYS | RIS_ODD)),
        // A ZWJ arms GB11 if a pictographic run precedes it, and is itself `InCB=Extend`,
        // so any conjunct sequence in progress carries on.
        CAT_ZWJ => mode | ((state & EXTPIC_RUN) >> 2) | (state & (ALWAYS | INCB_LINKED | INCB_RUN)),
        // GB9c only applies to extended clusters.
        // Never opening the run in legacy mode is what keeps `INCB_LINKED` clear there,
        // so the `INCB_LINKED` mask stays a boundary.
        // `EXTENDED >> 1 == INCB_RUN`, which opens the run without branching on the mode;
        // consonants are common in the scripts GB9c exists for, and the branch mispredicts.
        CAT_CONSONANT => mode | ALWAYS | (mode >> 1),
        _ => mode | ALWAYS,
    }
}

/// Forward-only driver over a contiguous string.
///
/// `GraphemeCursor` has to be able to recover the context that GB9c, GB11 and GB12/GB13
/// need by scanning backwards, because it may be shown the string in pieces and walked in either direction.
#[derive(Clone, Debug)]
struct GraphemeFwd<'a> {
    iter: core::str::CharIndices<'a>,
    /// Total length of the string.
    len: usize,
    /// Byte offset of the start of the cluster being scanned.
    offset: usize,
    /// Category of the last consumed codepoint.
    cat_before: u8,
    /// Packed rule state for everything consumed so far.
    ///
    /// The same value `GraphemeCursor::packed_state` holds, seeded by `initial_state` and
    /// advanced by `next_state` just the same.
    ///
    /// It needs no `Option` here because a forward-only driver always knows it.
    packed_state: u8,
}

impl<'a> GraphemeFwd<'a> {
    fn new(s: &'a str, is_extended: bool) -> GraphemeFwd<'a> {
        let mut fwd = GraphemeFwd {
            iter: s.char_indices(),
            len: s.len(),
            offset: 0,
            cat_before: GraphemeCat::GC_Any as u8,
            packed_state: initial_state(is_extended),
        };
        // Consume the first codepoint; nothing can break before it.
        if let Some((_, ch)) = fwd.iter.next() {
            fwd.cat_before = gr::grapheme_category_raw(ch);
            fwd.packed_state = next_state(fwd.packed_state, fwd.cat_before, ch);
        }
        fwd
    }

    /// Advance to the next boundary and return its offset.
    #[inline]
    fn next_boundary(&mut self) -> usize {
        loop {
            let (i, ch) = match self.iter.next() {
                Some(next) => next,
                None => {
                    self.offset = self.len;
                    return self.len;
                }
            };
            let cat_after = gr::grapheme_category_raw(ch);
            let boundary = self.packed_state
                & PAIR_MASK[(self.cat_before as usize) << 4 | cat_after as usize]
                == 0;
            self.packed_state = next_state(self.packed_state, cat_after, ch);
            self.cat_before = cat_after;
            if boundary {
                self.offset = i;
                return i;
            }
        }
    }
}

impl GraphemeCursor {
    /// Create a new cursor. The string and initial offset are given at creation
    /// time, but the contents of the string are not. The `is_extended` parameter
    /// controls whether extended grapheme clusters are selected.
    ///
    /// The `offset` parameter must be on a codepoint boundary.
    ///
    /// ```rust
    /// # use unicode_segmentation::GraphemeCursor;
    /// let s = "हिन्दी";
    /// let mut legacy = GraphemeCursor::new(0, s.len(), false);
    /// assert_eq!(legacy.next_boundary(s, 0), Ok(Some("ह".len())));
    /// let mut extended = GraphemeCursor::new(0, s.len(), true);
    /// assert_eq!(extended.next_boundary(s, 0), Ok(Some("हि".len())));
    /// ```
    pub fn new(offset: usize, len: usize, is_extended: bool) -> GraphemeCursor {
        let state = if offset == 0 || offset == len {
            GraphemeState::Break
        } else {
            GraphemeState::Unknown
        };
        GraphemeCursor {
            offset,
            len,
            state,
            is_extended,
            cat_before: None,
            cat_after: None,
            pre_context_offset: None,
            incb_linker_count: None,
            ris_count: None,
            packed_state: (offset == 0).then(|| initial_state(is_extended)),
            resuming: false,
        }
    }

    #[inline]
    fn grapheme_category(&self, ch: char) -> GraphemeCat {
        // Everything but a rare tail is a direct index into a category window,
        // so there is no range left worth caching between calls.
        gr::grapheme_category(ch)
    }

    // Not sure I'm gonna keep this, the advantage over new() seems thin.

    /// Set the cursor to a new location in the same string.
    ///
    /// ```rust
    /// # use unicode_segmentation::GraphemeCursor;
    /// let s = "abcd";
    /// let mut cursor = GraphemeCursor::new(0, s.len(), false);
    /// assert_eq!(cursor.cur_cursor(), 0);
    /// cursor.set_cursor(2);
    /// assert_eq!(cursor.cur_cursor(), 2);
    /// ```
    pub fn set_cursor(&mut self, offset: usize) {
        if offset != self.offset {
            self.offset = offset;
            self.state = if offset == 0 || offset == self.len {
                GraphemeState::Break
            } else {
                GraphemeState::Unknown
            };
            // reset state derived from text around cursor
            self.cat_before = None;
            self.cat_after = None;
            self.incb_linker_count = None;
            self.ris_count = None;
            self.packed_state = (offset == 0).then(|| initial_state(self.is_extended));
        }
    }

    #[inline]
    /// The current offset of the cursor. Equal to the last value provided to
    /// `new()` or `set_cursor()`, or returned from `next_boundary()` or
    /// `prev_boundary()`.
    ///
    /// ```rust
    /// # use unicode_segmentation::GraphemeCursor;
    /// // Two flags (🇷🇸🇮🇴), each flag is two RIS codepoints, each RIS is 4 bytes.
    /// let flags = "\u{1F1F7}\u{1F1F8}\u{1F1EE}\u{1F1F4}";
    /// let mut cursor = GraphemeCursor::new(4, flags.len(), false);
    /// assert_eq!(cursor.cur_cursor(), 4);
    /// assert_eq!(cursor.next_boundary(flags, 0), Ok(Some(8)));
    /// assert_eq!(cursor.cur_cursor(), 8);
    /// ```
    pub fn cur_cursor(&self) -> usize {
        self.offset
    }

    /// Provide additional pre-context when it is needed to decide a boundary.
    /// The end of the chunk must coincide with the value given in the
    /// `GraphemeIncomplete::PreContext` request.
    ///
    /// ```rust
    /// # use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
    /// let flags = "\u{1F1F7}\u{1F1F8}\u{1F1EE}\u{1F1F4}";
    /// let mut cursor = GraphemeCursor::new(8, flags.len(), false);
    /// // Not enough pre-context to decide if there's a boundary between the two flags.
    /// assert_eq!(cursor.is_boundary(&flags[8..], 8), Err(GraphemeIncomplete::PreContext(8)));
    /// // Provide one more Regional Indicator Symbol of pre-context
    /// cursor.provide_context(&flags[4..8], 4);
    /// // Still not enough context to decide.
    /// assert_eq!(cursor.is_boundary(&flags[8..], 8), Err(GraphemeIncomplete::PreContext(4)));
    /// // Provide additional requested context.
    /// cursor.provide_context(&flags[0..4], 0);
    /// // That's enough to decide (it always is when context goes to the start of the string)
    /// assert_eq!(cursor.is_boundary(&flags[8..], 8), Ok(true));
    /// ```
    pub fn provide_context(&mut self, chunk: &str, chunk_start: usize) {
        use crate::tables::grapheme as gr;
        assert!(chunk_start.saturating_add(chunk.len()) == self.pre_context_offset.unwrap());
        self.pre_context_offset = None;
        if self.is_extended && chunk_start + chunk.len() == self.offset {
            let ch = chunk.chars().next_back().unwrap();
            if self.grapheme_category(ch) == gr::GC_Prepend {
                self.decide(false); // GB9b
                return;
            }
        }
        match self.state {
            GraphemeState::InCbConsonant => self.handle_incb_consonant(chunk, chunk_start),
            GraphemeState::Regional => self.handle_regional(chunk, chunk_start),
            GraphemeState::Emoji { seen_zwj } => self.handle_emoji(chunk, chunk_start, seen_zwj),
            _ => {
                if self.cat_before.is_none() && self.offset == chunk.len() + chunk_start {
                    let ch = chunk.chars().next_back().unwrap();
                    self.cat_before = Some(self.grapheme_category(ch));
                }
            }
        }
    }

    /// Advance the RIS and `InCB=Linker` counters over one consumed codepoint.
    ///
    /// The packed state supersedes these for deciding a boundary, but they are still what
    /// the backward scans resume from and what `is_boundary` consults at a chunk start.
    #[inline]
    fn step_counters(&mut self, cat: u8, ch: char) {
        // Every `InCB=Linker` is `gcb=Extend`, so a category that is neither `Extend` nor
        // `ZWJ` rules out both InCB roles without touching the codepoint.
        if cat != CAT_EXTEND && cat != CAT_ZWJ {
            self.incb_linker_count = Some(0);
        } else if crate::tables::is_incb_linker(ch) {
            self.incb_linker_count = Some(self.incb_linker_count.map_or(1, |c| c + 1));
        } else if ch == '\u{200c}' {
            self.incb_linker_count = Some(0);
        }
        if cat == CAT_REGIONAL {
            self.ris_count = self.ris_count.map(|c| c + 1);
        } else {
            self.ris_count = Some(0);
        }
    }

    /// Walk forward from `self.offset` to the next boundary,
    /// with every rule resolved by the packed state.
    ///
    /// `chunk` must be the whole string and `packed_state` the state at `self.offset`.
    ///
    /// Each codepoint costs a category lookup, a mask test and a state update;
    /// the backward scans and the `Option` bookkeeping the general path needs are
    /// all unnecessary once the state is known.
    ///
    /// Leaves the cursor exactly as the general path would.
    fn scan_boundary(&mut self, chunk: &str, packed_state: u8) -> usize {
        let mut packed_state = packed_state;
        let mut iter = chunk[self.offset..].chars();
        let mut ch = iter.next().expect("offset < len, so a codepoint follows");
        let mut cat = match self.cat_after {
            Some(cat) => cat as u8,
            None => gr::grapheme_category_raw(ch),
        };
        loop {
            self.offset += ch.len_utf8();
            packed_state = next_state(packed_state, cat, ch);
            self.step_counters(cat, ch);
            let cat_before = cat;
            match iter.next() {
                Some(next_ch) => {
                    ch = next_ch;
                    cat = gr::grapheme_category_raw(ch);
                    if packed_state & PAIR_MASK[(cat_before as usize) << 4 | cat as usize] == 0 {
                        self.state = GraphemeState::Break;
                        self.cat_before = Some(gr::CATS[cat_before as usize]);
                        self.cat_after = Some(gr::CATS[cat as usize]);
                        self.packed_state = Some(packed_state);
                        return self.offset;
                    }
                }
                None => {
                    // End of the string is always a boundary.
                    self.state = GraphemeState::Break;
                    self.cat_before = Some(gr::CATS[cat_before as usize]);
                    self.cat_after = None;
                    self.packed_state = Some(packed_state);
                    return self.offset;
                }
            }
        }
    }

    #[inline]
    fn decide(&mut self, is_break: bool) {
        self.state = if is_break {
            GraphemeState::Break
        } else {
            GraphemeState::NotBreak
        };
    }

    #[inline]
    fn decision(&mut self, is_break: bool) -> Result<bool, GraphemeIncomplete> {
        self.decide(is_break);
        Ok(is_break)
    }

    #[inline]
    fn is_boundary_result(&self) -> Result<bool, GraphemeIncomplete> {
        if self.state == GraphemeState::Break {
            Ok(true)
        } else if self.state == GraphemeState::NotBreak {
            Ok(false)
        } else if let Some(pre_context_offset) = self.pre_context_offset {
            Err(GraphemeIncomplete::PreContext(pre_context_offset))
        } else {
            unreachable!("inconsistent state");
        }
    }

    /// For handling rule GB9c:
    ///
    /// There's an `InCB=Consonant` after this, and we need to look back
    /// to verify whether there should be a break.
    ///
    /// Seek backward to find an `InCB=Linker` preceded by an `InCB=Consonsnt`
    /// (potentially separated by some number of `InCB=Linker` or `InCB=Extend`).
    /// If we find the consonant in question, then there's no break; if we find a consonant
    /// with no linker, or a non-linker non-extend non-consonant, or the start of text, there's a break;
    /// otherwise we need more context
    #[inline]
    fn handle_incb_consonant(&mut self, chunk: &str, chunk_start: usize) {
        use crate::tables::{self, grapheme as gr};

        // GB9c only applies to extended grapheme clusters
        if !self.is_extended {
            self.decide(true);
            return;
        }

        let mut incb_linker_count = self.incb_linker_count.unwrap_or(0);

        for ch in chunk.chars().rev() {
            let cat = self.grapheme_category(ch);
            if is_incb_extend(cat, ch) {
                // We ignore InCB extends, continue
                continue;
            }
            if tables::is_incb_linker(ch) {
                // We found an InCB linker
                incb_linker_count += 1;
                self.incb_linker_count = Some(incb_linker_count);
            } else {
                // Prev character is neither linker nor extend, break suppressed iff it's InCB=Consonant
                let result = !(incb_linker_count > 0 && cat == gr::GC_InCB_Consonant);
                self.decide(result);
                return;
            }
        }

        if chunk_start == 0 {
            // Start of text and we still haven't found a consonant, so break
            self.decide(true);
        } else {
            // We need more context
            self.pre_context_offset = Some(chunk_start);
            self.state = GraphemeState::InCbConsonant;
        }
    }

    #[inline]
    fn handle_regional(&mut self, chunk: &str, chunk_start: usize) {
        let mut ris_count = self.ris_count.unwrap_or(0);
        for ch in chunk.chars().rev() {
            if self.grapheme_category(ch) != gr::GC_Regional_Indicator {
                self.ris_count = Some(ris_count);
                self.decide(ris_count % 2 == 0);
                return;
            }
            ris_count += 1;
        }
        self.ris_count = Some(ris_count);
        if chunk_start == 0 {
            self.decide(ris_count % 2 == 0);
        } else {
            self.pre_context_offset = Some(chunk_start);
            self.state = GraphemeState::Regional;
        }
    }

    #[inline]
    fn handle_emoji(&mut self, chunk: &str, chunk_start: usize, mut seen_zwj: bool) {
        // \p{Extended_Pictographic} Extend* ZWJ 	× 	\p{Extended_Pictographic}
        use crate::tables::grapheme as gr;
        let mut iter = chunk.chars().rev();
        if !seen_zwj {
            if let Some(ch) = iter.next() {
                if self.grapheme_category(ch) != gr::GC_ZWJ {
                    self.decide(true);
                    return;
                } else {
                    seen_zwj = true;
                }
            }
        }
        for ch in iter {
            match self.grapheme_category(ch) {
                gr::GC_Extend => (),
                gr::GC_Extended_Pictographic => {
                    self.decide(false);
                    return;
                }
                _ => {
                    self.decide(true);
                    return;
                }
            }
        }
        if chunk_start == 0 {
            self.decide(true);
        } else {
            self.pre_context_offset = Some(chunk_start);
            self.state = GraphemeState::Emoji { seen_zwj };
        }
    }

    #[inline]
    /// Determine whether the current cursor location is a grapheme cluster boundary.
    /// Only a part of the string need be supplied. If `chunk_start` is nonzero or
    /// the length of `chunk` is not equal to `len` on creation, then this method
    /// may return `GraphemeIncomplete::PreContext`. The caller should then
    /// call `provide_context` with the requested chunk, then retry calling this
    /// method.
    ///
    /// For partial chunks, if the cursor is not at the beginning or end of the
    /// string, the chunk should contain at least the codepoint following the cursor.
    /// If the string is nonempty, the chunk must be nonempty.
    ///
    /// All calls should have consistent chunk contents (ie, if a chunk provides
    /// content for a given slice, all further chunks covering that slice must have
    /// the same content for it).
    ///
    /// ```rust
    /// # use unicode_segmentation::GraphemeCursor;
    /// let flags = "\u{1F1F7}\u{1F1F8}\u{1F1EE}\u{1F1F4}";
    /// let mut cursor = GraphemeCursor::new(8, flags.len(), false);
    /// assert_eq!(cursor.is_boundary(flags, 0), Ok(true));
    /// cursor.set_cursor(12);
    /// assert_eq!(cursor.is_boundary(flags, 0), Ok(false));
    /// ```
    pub fn is_boundary(
        &mut self,
        chunk: &str,
        chunk_start: usize,
    ) -> Result<bool, GraphemeIncomplete> {
        use crate::tables::grapheme as gr;
        if self.state == GraphemeState::Break {
            return Ok(true);
        }
        if self.state == GraphemeState::NotBreak {
            return Ok(false);
        }
        if (self.offset < chunk_start || self.offset >= chunk_start.saturating_add(chunk.len()))
            && (self.offset > chunk_start.saturating_add(chunk.len()) || self.cat_after.is_none())
        {
            return Err(GraphemeIncomplete::InvalidOffset);
        }
        if let Some(pre_context_offset) = self.pre_context_offset {
            return Err(GraphemeIncomplete::PreContext(pre_context_offset));
        }
        let offset_in_chunk = self.offset.saturating_sub(chunk_start);
        if self.cat_after.is_none() {
            let ch = chunk[offset_in_chunk..].chars().next().unwrap();
            self.cat_after = Some(self.grapheme_category(ch));
        }
        if self.offset == chunk_start {
            let mut need_pre_context = true;
            match self.cat_after.unwrap() {
                gr::GC_InCB_Consonant => self.state = GraphemeState::InCbConsonant,
                // Only look back for the RI count if it isn't known already.
                gr::GC_Regional_Indicator if self.ris_count.is_none() => {
                    self.state = GraphemeState::Regional
                }
                gr::GC_Extended_Pictographic => {
                    self.state = GraphemeState::Emoji { seen_zwj: false }
                }
                _ => need_pre_context = self.cat_before.is_none(),
            }
            if need_pre_context {
                self.pre_context_offset = Some(chunk_start);
                return Err(GraphemeIncomplete::PreContext(chunk_start));
            }
        }
        if self.cat_before.is_none() {
            let ch = chunk[..offset_in_chunk].chars().next_back().unwrap();
            self.cat_before = Some(self.grapheme_category(ch));
        }
        let (before, after) = (self.cat_before.unwrap(), self.cat_after.unwrap());
        match self.packed_state {
            // The packed state already summarises the pre-context every rule cares about,
            // so a single mask test replaces `check_pair` and the backward scans below.
            //
            // Only when the chunk starts the string, though: from there a backward scan can
            // never run out of chunk, so skipping it cannot turn a `PreContext` request into an answer,
            // and callers feeding the cursor in pieces see the protocol they always have.
            //
            // `ris_count` must already be known for the same reason as in `next_boundary`:
            // `handle_regional` is the only scan whose side effect is visible later.
            Some(packed_state) if chunk_start == 0 && self.ris_count.is_some() => self
                .decision(packed_state & PAIR_MASK[(before as usize) << 4 | after as usize] == 0),
            _ => match check_pair(before, after) {
                PairResult::NotBreak => self.decision(false),
                PairResult::Break => self.decision(true),
                PairResult::Extended => {
                    let is_extended = self.is_extended;
                    self.decision(!is_extended)
                }
                PairResult::InCbConsonant => {
                    self.handle_incb_consonant(&chunk[..offset_in_chunk], chunk_start);
                    self.is_boundary_result()
                }
                PairResult::Regional => {
                    if let Some(ris_count) = self.ris_count {
                        return self.decision((ris_count % 2) == 0);
                    }
                    self.handle_regional(&chunk[..offset_in_chunk], chunk_start);
                    self.is_boundary_result()
                }
                PairResult::Emoji => {
                    self.handle_emoji(&chunk[..offset_in_chunk], chunk_start, false);
                    self.is_boundary_result()
                }
            },
        }
    }

    #[inline]
    /// Find the next boundary after the current cursor position. Only a part of
    /// the string need be supplied. If the chunk is incomplete, then this
    /// method might return `GraphemeIncomplete::PreContext` or
    /// `GraphemeIncomplete::NextChunk`. In the former case, the caller should
    /// call `provide_context` with the requested chunk, then retry. In the
    /// latter case, the caller should provide the chunk following the one
    /// given, then retry.
    ///
    /// See `is_boundary` for expectations on the provided chunk.
    ///
    /// ```rust
    /// # use unicode_segmentation::GraphemeCursor;
    /// let flags = "\u{1F1F7}\u{1F1F8}\u{1F1EE}\u{1F1F4}";
    /// let mut cursor = GraphemeCursor::new(4, flags.len(), false);
    /// assert_eq!(cursor.next_boundary(flags, 0), Ok(Some(8)));
    /// assert_eq!(cursor.next_boundary(flags, 0), Ok(Some(16)));
    /// assert_eq!(cursor.next_boundary(flags, 0), Ok(None));
    /// ```
    ///
    /// And an example that uses partial strings:
    ///
    /// ```rust
    /// # use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
    /// let s = "abcd";
    /// let mut cursor = GraphemeCursor::new(0, s.len(), false);
    /// assert_eq!(cursor.next_boundary(&s[..2], 0), Ok(Some(1)));
    /// assert_eq!(cursor.next_boundary(&s[..2], 0), Err(GraphemeIncomplete::NextChunk));
    /// assert_eq!(cursor.next_boundary(&s[2..4], 2), Ok(Some(2)));
    /// assert_eq!(cursor.next_boundary(&s[2..4], 2), Ok(Some(3)));
    /// assert_eq!(cursor.next_boundary(&s[2..4], 2), Ok(Some(4)));
    /// assert_eq!(cursor.next_boundary(&s[2..4], 2), Ok(None));
    /// ```
    pub fn next_boundary(
        &mut self,
        chunk: &str,
        chunk_start: usize,
    ) -> Result<Option<usize>, GraphemeIncomplete> {
        if self.offset == self.len {
            return Ok(None);
        }
        // Fast path: the whole string is in hand and the packed state is known,
        // so no rule can need pre-context and none of the suspend/resume bookkeeping can come into play.
        //
        // That is the shape `Graphemes` and `GraphemeIndices` always call in,
        // and a cursor walked forward from the start of a contiguous string as well.
        //
        // `handle_regional` seeds `ris_count` as a side effect, and whether it has run is observable at a chunk start,
        // so leave the leading run of regional indicators to the general path.
        if chunk_start == 0 && chunk.len() == self.len && !self.resuming && self.ris_count.is_some()
        {
            if let Some(packed_state) = self.packed_state {
                return Ok(Some(self.scan_boundary(chunk, packed_state)));
            }
        }
        let mut iter = chunk[self.offset.saturating_sub(chunk_start)..].chars();
        let mut ch = match iter.next() {
            Some(ch) => ch,
            None => return Err(GraphemeIncomplete::NextChunk),
        };
        loop {
            if self.resuming {
                if self.cat_after.is_none() {
                    self.cat_after = Some(self.grapheme_category(ch));
                }
            } else {
                self.offset = self.offset.saturating_add(ch.len_utf8());
                self.state = GraphemeState::Unknown;
                self.cat_before = self.cat_after.take();
                if self.cat_before.is_none() {
                    self.cat_before = Some(self.grapheme_category(ch));
                }
                let cat = self.cat_before.unwrap();
                self.step_counters(cat as u8, ch);
                // Walking forwards also keeps the packed state exact,
                // which is what lets `is_boundary` settle the stateful rules without scanning back.
                self.packed_state = self
                    .packed_state
                    .map(|packed_state| next_state(packed_state, cat as u8, ch));
                if let Some(next_ch) = iter.next() {
                    ch = next_ch;
                    self.cat_after = Some(self.grapheme_category(ch));
                } else if self.offset == self.len {
                    self.decide(true);
                } else {
                    self.resuming = true;
                    return Err(GraphemeIncomplete::NextChunk);
                }
            }
            self.resuming = true;
            if self.is_boundary(chunk, chunk_start)? {
                self.resuming = false;
                return Ok(Some(self.offset));
            }
            self.resuming = false;
        }
    }

    /// Find the previous boundary after the current cursor position. Only a part
    /// of the string need be supplied. If the chunk is incomplete, then this
    /// method might return `GraphemeIncomplete::PreContext` or
    /// `GraphemeIncomplete::PrevChunk`. In the former case, the caller should
    /// call `provide_context` with the requested chunk, then retry. In the
    /// latter case, the caller should provide the chunk preceding the one
    /// given, then retry.
    ///
    /// See `is_boundary` for expectations on the provided chunk.
    ///
    /// ```rust
    /// # use unicode_segmentation::GraphemeCursor;
    /// let flags = "\u{1F1F7}\u{1F1F8}\u{1F1EE}\u{1F1F4}";
    /// let mut cursor = GraphemeCursor::new(12, flags.len(), false);
    /// assert_eq!(cursor.prev_boundary(flags, 0), Ok(Some(8)));
    /// assert_eq!(cursor.prev_boundary(flags, 0), Ok(Some(0)));
    /// assert_eq!(cursor.prev_boundary(flags, 0), Ok(None));
    /// ```
    ///
    /// And an example that uses partial strings (note the exact return is not
    /// guaranteed, and may be `PrevChunk` or `PreContext` arbitrarily):
    ///
    /// ```rust
    /// # use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
    /// let s = "abcd";
    /// let mut cursor = GraphemeCursor::new(4, s.len(), false);
    /// assert_eq!(cursor.prev_boundary(&s[2..4], 2), Ok(Some(3)));
    /// assert_eq!(cursor.prev_boundary(&s[2..4], 2), Err(GraphemeIncomplete::PrevChunk));
    /// assert_eq!(cursor.prev_boundary(&s[0..2], 0), Ok(Some(2)));
    /// assert_eq!(cursor.prev_boundary(&s[0..2], 0), Ok(Some(1)));
    /// assert_eq!(cursor.prev_boundary(&s[0..2], 0), Ok(Some(0)));
    /// assert_eq!(cursor.prev_boundary(&s[0..2], 0), Ok(None));
    /// ```
    pub fn prev_boundary(
        &mut self,
        chunk: &str,
        chunk_start: usize,
    ) -> Result<Option<usize>, GraphemeIncomplete> {
        if self.offset == 0 {
            return Ok(None);
        }
        if self.offset == chunk_start {
            return Err(GraphemeIncomplete::PrevChunk);
        }
        let mut iter = chunk[..self.offset.saturating_sub(chunk_start)]
            .chars()
            .rev();
        let mut ch = iter.next().unwrap();
        loop {
            if self.offset == chunk_start {
                self.resuming = true;
                return Err(GraphemeIncomplete::PrevChunk);
            }
            if self.resuming {
                self.cat_before = Some(self.grapheme_category(ch));
            } else {
                self.offset -= ch.len_utf8();
                self.cat_after = self.cat_before.take();
                self.state = GraphemeState::Unknown;
                // The packed state only runs forwards,
                // so it is unknown from here and the backward scans decide the stateful rules again.
                self.packed_state = (self.offset == 0).then(|| initial_state(self.is_extended));
                if let Some(incb_linker_count) = self.incb_linker_count {
                    self.incb_linker_count =
                        if incb_linker_count > 0 && crate::tables::is_incb_linker(ch) {
                            Some(incb_linker_count - 1)
                        } else if is_incb_extend(self.grapheme_category(ch), ch) {
                            Some(incb_linker_count)
                        } else {
                            None
                        };
                }
                if let Some(ris_count) = self.ris_count {
                    self.ris_count = if ris_count > 0 {
                        Some(ris_count - 1)
                    } else {
                        None
                    };
                }
                if let Some(prev_ch) = iter.next() {
                    ch = prev_ch;
                    self.cat_before = Some(self.grapheme_category(ch));
                } else if self.offset == 0 {
                    self.decide(true);
                } else {
                    self.resuming = true;
                    self.cat_after = Some(self.grapheme_category(ch));
                    return Err(GraphemeIncomplete::PrevChunk);
                }
            }
            self.resuming = true;
            if self.is_boundary(chunk, chunk_start)? {
                self.resuming = false;
                return Ok(Some(self.offset));
            }
            self.resuming = false;
        }
    }
}

#[test]
fn test_grapheme_cursor_ris_count_across_chunks() {
    use GraphemeIncomplete::*;

    let chunk0 = "a"; // 1 byte
    let chunk1 = "\u{1f1e6}"; // 4 bytes
    let chunk2 = "\u{1f1e6}"; // 4 bytes
    let full_len = chunk0.len() + chunk1.len() + chunk2.len(); // 9
    let chunk1_start = chunk0.len();
    let chunk2_start = chunk0.len() + chunk1.len();

    let mut c = GraphemeCursor::new(0, full_len, true);
    assert_eq!(c.next_boundary(chunk0, 0), Err(NextChunk));
    assert_eq!(c.next_boundary(chunk1, chunk1_start), Ok(Some(1)));
    assert_eq!(c.next_boundary(chunk1, chunk1_start), Err(NextChunk));
    assert_eq!(c.next_boundary(chunk2, chunk2_start), Ok(Some(9)));
    assert_eq!(c.next_boundary(chunk2, chunk2_start), Ok(None));
}

#[test]
fn test_grapheme_cursor_ris_precontext() {
    let s = "\u{1f1fa}\u{1f1f8}\u{1f1fa}\u{1f1f8}\u{1f1fa}\u{1f1f8}";
    let mut c = GraphemeCursor::new(8, s.len(), true);
    assert_eq!(
        c.is_boundary(&s[4..], 4),
        Err(GraphemeIncomplete::PreContext(4))
    );
    c.provide_context(&s[..4], 0);
    assert_eq!(c.is_boundary(&s[4..], 4), Ok(true));
}

#[test]
fn test_grapheme_cursor_chunk_start_require_precontext() {
    let s = "\r\n";
    let mut c = GraphemeCursor::new(1, s.len(), true);
    assert_eq!(
        c.is_boundary(&s[1..], 1),
        Err(GraphemeIncomplete::PreContext(1))
    );
    c.provide_context(&s[..1], 0);
    assert_eq!(c.is_boundary(&s[1..], 1), Ok(false));
}

#[test]
fn test_grapheme_cursor_prev_boundary() {
    let s = "abcd";
    let mut c = GraphemeCursor::new(3, s.len(), true);
    assert_eq!(
        c.prev_boundary(&s[2..], 2),
        Err(GraphemeIncomplete::PrevChunk)
    );
    assert_eq!(c.prev_boundary(&s[..2], 0), Ok(Some(2)));
}

#[test]
fn test_grapheme_cursor_prev_boundary_chunk_start() {
    let s = "abcd";
    let mut c = GraphemeCursor::new(2, s.len(), true);
    assert_eq!(
        c.prev_boundary(&s[2..], 2),
        Err(GraphemeIncomplete::PrevChunk)
    );
    assert_eq!(c.prev_boundary(&s[..2], 0), Ok(Some(1)));
}

#[test]
fn test_grapheme_cursor_boundary_with_zwj_on_chunk_start() {
    use GraphemeIncomplete::*;

    let chunk0 = "👩"; // 4 bytes
    let chunk1 = "\u{200d}🔬"; // 3 bytes + 4 bytes

    let full_len = chunk0.len() + chunk1.len();

    let mut cur = GraphemeCursor::new(0, full_len, true);
    assert_eq!(cur.next_boundary(chunk0, 0), Err(NextChunk));
    match cur.next_boundary(chunk1, chunk0.len()) {
        Ok(res) => assert_eq!(res, Some(11)),
        Err(PreContext(_)) => {
            cur.provide_context(chunk0, 0);
            assert_eq!(cur.next_boundary(chunk1, chunk0.len()), Ok(Some(11)));
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_grapheme_cursor_emoji_no_zwj() {
    use GraphemeIncomplete::*;
    let chunk0 = "🍒"; // 4 bytes
    let chunk1 = "🥑"; // 4 bytes
    let full_len = chunk0.len() + chunk1.len();

    let mut c = GraphemeCursor::new(0, full_len, true);
    assert_eq!(c.next_boundary(chunk0, 0), Err(NextChunk));
    assert_eq!(
        c.next_boundary(chunk1, chunk0.len()),
        Err(PreContext(chunk0.len()))
    );
    c.provide_context(chunk0, 0);
    assert_eq!(c.next_boundary(chunk1, chunk0.len()), Ok(Some(4)));
    assert_eq!(c.next_boundary(chunk1, chunk0.len()), Ok(Some(8)));
    assert_eq!(c.next_boundary(chunk1, chunk0.len()), Ok(None));
}

#[test]
fn test_grapheme_cursor_emoji_chunk_boundary_before_zwj() {
    use GraphemeIncomplete::*;
    let chunk0 = "🍒"; // 4 bytes
    let chunk1 = "\u{200d}🥑"; // 3 + 4 bytes
    let full_len = chunk0.len() + chunk1.len(); // 11

    let mut c = GraphemeCursor::new(0, full_len, true);
    assert_eq!(c.next_boundary(chunk0, 0), Err(NextChunk));
    assert_eq!(
        c.next_boundary(chunk1, chunk0.len()),
        Err(PreContext(chunk0.len()))
    );
    c.provide_context(chunk0, 0);
    assert_eq!(c.next_boundary(chunk1, chunk0.len()), Ok(Some(11)));
    assert_eq!(c.next_boundary(chunk1, chunk0.len()), Ok(None));
}

#[test]
fn test_grapheme_cursor_emoji_chunk_boundary_after_zwj() {
    use GraphemeIncomplete::*;
    let chunk0 = "🍒\u{200d}"; // 4 + 3 bytes
    let chunk1 = "🥑"; // 4 bytes
    let full_len = chunk0.len() + chunk1.len(); // 11

    let mut c = GraphemeCursor::new(0, full_len, true);
    assert_eq!(c.next_boundary(chunk0, 0), Err(NextChunk));
    assert_eq!(
        c.next_boundary(chunk1, chunk0.len()),
        Err(PreContext(chunk0.len()))
    );
    c.provide_context(chunk0, 0);
    assert_eq!(c.next_boundary(chunk1, chunk0.len()), Ok(Some(11)));
    assert_eq!(c.next_boundary(chunk1, chunk0.len()), Ok(None));
}

#[test]
fn test_grapheme_cursor_emoji_zwj_across_chunks() {
    use GraphemeIncomplete::*;
    let chunk0 = "🍒"; // 4 bytes
    let chunk1 = "\u{200d}"; // 3 bytes
    let chunk2 = "🥑"; // 4 bytes
    let full_len = chunk0.len() + chunk1.len() + chunk2.len(); // 11
    let chunk2_start = chunk0.len() + chunk1.len();

    let mut c = GraphemeCursor::new(0, full_len, true);
    assert_eq!(c.next_boundary(chunk0, 0), Err(NextChunk));
    assert_eq!(c.next_boundary(chunk1, chunk0.len()), Err(NextChunk));
    assert_eq!(
        c.next_boundary(chunk2, chunk2_start),
        Err(PreContext(chunk2_start))
    );
    c.provide_context(chunk1, chunk0.len());
    assert_eq!(
        c.next_boundary(chunk2, chunk2_start),
        Err(PreContext(chunk0.len()))
    );
    c.provide_context(chunk0, 0);
    assert_eq!(c.next_boundary(chunk2, chunk2_start), Ok(Some(11)));
    assert_eq!(c.next_boundary(chunk2, chunk2_start), Ok(None));
}
