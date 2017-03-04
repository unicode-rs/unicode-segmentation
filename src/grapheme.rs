// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::cmp;

use tables::grapheme::GraphemeCat;

/// External iterator for grapheme clusters and byte offsets.
#[derive(Clone)]
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
        self.iter.next().map(|s| (s.as_ptr() as usize - self.start_offset, s))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> DoubleEndedIterator for GraphemeIndices<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<(usize, &'a str)> {
        self.iter.next_back().map(|s| (s.as_ptr() as usize - self.start_offset, s))
    }
}

/// External iterator for a string's
/// [grapheme clusters](http://www.unicode.org/reports/tr29/#Grapheme_Cluster_Boundaries).
#[derive(Clone)]
pub struct Graphemes<'a> {
    string: &'a str,
    extended: bool,
    cat: Option<GraphemeCat>,
    catb: Option<GraphemeCat>,
    regional_count_back: Option<usize>,
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
        self.string
    }
}

// state machine for cluster boundary rules
#[derive(Copy,Clone,PartialEq,Eq)]
enum GraphemeState {
    Start,
    FindExtend,
    HangulL,
    HangulLV,
    HangulLVT,
    Prepend,
    Regional,
    Emoji,
    Zwj,
    Unknown,
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = &'a str;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let slen = self.string.len();
        (cmp::min(slen, 1), Some(slen))
    }

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        use self::GraphemeState::*;
        use tables::grapheme as gr;
        if self.string.len() == 0 {
            return None;
        }

        let mut take_curr = true;
        let mut idx = 0;
        let mut state = Start;
        let mut cat = gr::GC_Any;

        // caching used by next_back() should be invalidated
        self.regional_count_back = None;
        self.catb = None;

        for (curr, ch) in self.string.char_indices() {
            idx = curr;

            // retrieve cached category, if any
            // We do this because most of the time we would end up
            // looking up each character twice.
            cat = match self.cat {
                None => gr::grapheme_category(ch),
                _ => self.cat.take().unwrap()
            };

            if (state, cat) == (Emoji, gr::GC_Extend) {
                continue;                   // rule GB10
            }

            if let Some(new_state) = match cat {
                gr::GC_Extend => Some(FindExtend),                       // rule GB9
                gr::GC_SpacingMark if self.extended => Some(FindExtend), // rule GB9a
                gr::GC_ZWJ => Some(Zwj),                                 // rule GB9/GB11
                _ => None
            } {
                state = new_state;
                continue;
            }

            state = match state {
                Start if '\r' == ch => {
                    let slen = self.string.len();
                    let nidx = idx + 1;
                    if nidx != slen && self.string[nidx..].chars().next().unwrap() == '\n' {
                        idx = nidx;             // rule GB3
                    }
                    break;                      // rule GB4
                }
                Start | Prepend => match cat {
                    gr::GC_Control => {         // rule GB5
                        take_curr = state == Start;
                        break;
                    }
                    gr::GC_L => HangulL,
                    gr::GC_LV | gr::GC_V => HangulLV,
                    gr::GC_LVT | gr::GC_T => HangulLVT,
                    gr::GC_Prepend if self.extended => Prepend,
                    gr::GC_Regional_Indicator => Regional,
                    gr::GC_E_Base | gr::GC_E_Base_GAZ => Emoji,
                    _ => FindExtend
                },
                FindExtend => {         // found non-extending when looking for extending
                    take_curr = false;
                    break;
                },
                HangulL => match cat {      // rule GB6: L x (L|V|LV|LVT)
                    gr::GC_L => continue,
                    gr::GC_LV | gr::GC_V => HangulLV,
                    gr::GC_LVT => HangulLVT,
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                HangulLV => match cat {     // rule GB7: (LV|V) x (V|T)
                    gr::GC_V => continue,
                    gr::GC_T => HangulLVT,
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                HangulLVT => match cat {    // rule GB8: (LVT|T) x T
                    gr::GC_T => continue,
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                Regional => match cat {     // rule GB12/GB13
                    gr::GC_Regional_Indicator => FindExtend,
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                Emoji => match cat {        // rule GB10: (E_Base|EBG) Extend* x E_Modifier
                    gr::GC_E_Modifier => continue,
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                Zwj => match cat {          // rule GB11: ZWJ x (GAZ|EBG)
                    gr::GC_Glue_After_Zwj => continue,
                    gr::GC_E_Base_GAZ => Emoji,
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                Unknown => unreachable!(),
            }
        }

        self.cat = if take_curr {
            idx = idx + self.string[idx..].chars().next().unwrap().len_utf8();
            None
        } else {
            Some(cat)
        };

        let retstr = &self.string[..idx];
        self.string = &self.string[idx..];
        Some(retstr)
    }
}

impl<'a> DoubleEndedIterator for Graphemes<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        use self::GraphemeState::*;
        use tables::grapheme as gr;
        if self.string.len() == 0 {
            return None;
        }

        let mut take_curr = true;
        let mut idx = self.string.len();
        let mut previdx = idx;
        let mut state = Start;
        let mut cat = gr::GC_Any;

        // caching used by next() should be invalidated
        self.cat = None;

        'outer: for (curr, ch) in self.string.char_indices().rev() {
            previdx = idx;
            idx = curr;

            // cached category, if any
            cat = match self.catb {
                None => gr::grapheme_category(ch),
                _ => self.catb.take().unwrap()
            };

            // a matching state machine that runs *backwards* across an input string
            // note that this has some implications for the Hangul matching, since
            // we now need to know what the rightward letter is:
            //
            // Right to left, we have:
            //      L x L
            //      V x (L|V|LV)
            //      T x (V|T|LV|LVT)
            // HangulL means the letter to the right is L
            // HangulLV means the letter to the right is V
            // HangulLVT means the letter to the right is T
            state = match state {
                Start if '\n' == ch => {
                    if idx > 0 && '\r' == self.string[..idx].chars().next_back().unwrap() {
                        idx -= 1;       // rule GB3
                    }
                    break;              // rule GB4
                },
                Start | FindExtend => match cat {
                    gr::GC_Extend => FindExtend,
                    gr::GC_SpacingMark if self.extended => FindExtend,
                    gr::GC_ZWJ => FindExtend,
                    gr::GC_E_Modifier => Emoji,
                    gr::GC_Glue_After_Zwj | gr::GC_E_Base_GAZ => Zwj,
                    gr::GC_L | gr::GC_LV | gr::GC_LVT => HangulL,
                    gr::GC_V => HangulLV,
                    gr::GC_T => HangulLVT,
                    gr::GC_Regional_Indicator => Regional,
                    gr::GC_Control => {
                        take_curr = Start == state;
                        break;
                    },
                    _ => break
                },
                HangulL => match cat {      // char to right is an L
                    gr::GC_L => continue,               // L x L is the only legal match
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                HangulLV => match cat {     // char to right is a V
                    gr::GC_V => continue,               // V x V, right char is still V
                    gr::GC_L | gr::GC_LV => HangulL,    // (L|V) x V, right char is now L
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                HangulLVT => match cat {    // char to right is a T
                    gr::GC_T => continue,               // T x T, right char is still T
                    gr::GC_V => HangulLV,               // V x T, right char is now V
                    gr::GC_LV | gr::GC_LVT => HangulL,  // (LV|LVT) x T, right char is now L
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                Prepend => {
                    // not used in reverse iteration
                    unreachable!()
                },
                Regional => {               // rule GB12/GB13
                    // Need to scan backward to find if this is preceded by an odd or even number
                    // of Regional_Indicator characters.
                    let count = match self.regional_count_back {
                        Some(count) => count,
                        None => self.string[..previdx].chars().rev().take_while(|c| {
                                    gr::grapheme_category(*c) == gr::GC_Regional_Indicator
                                }).count()
                    };
                    // Cache the count to avoid re-scanning the same chars on the next iteration.
                    self.regional_count_back = count.checked_sub(1);

                    if count % 2 == 0 {
                        take_curr = false;
                        break;
                    }
                    continue;
                },
                Emoji => {                  // char to right is E_Modifier
                    // In order to decide whether to break before this E_Modifier char, we need to
                    // scan backward past any Extend chars to look for (E_Base|(ZWJ? EBG)).
                    let mut ebg_idx = None;
                    for (startidx, prev) in self.string[..previdx].char_indices().rev() {
                        match (ebg_idx, gr::grapheme_category(prev)) {
                            (None, gr::GC_Extend) => continue,
                            (None, gr::GC_E_Base) => {      // rule GB10
                                // Found an Emoji modifier sequence. Return the whole sequence.
                                idx = startidx;
                                break 'outer;
                            }
                            (None, gr::GC_E_Base_GAZ) => {  // rule GB10
                                // Keep scanning in case this is part of an ZWJ x EBJ pair.
                                ebg_idx = Some(startidx);
                            }
                            (Some(_), gr::GC_ZWJ) => {      // rule GB11
                                idx = startidx;
                                break 'outer;
                            }
                            _ => break
                        }
                    }
                    if let Some(ebg_idx) = ebg_idx {
                        // Found an EBG without a ZWJ before it.
                        idx = ebg_idx;
                        break;
                    }
                    // Not part of an Emoji modifier sequence. Break here.
                    take_curr = false;
                    break;
                },
                Zwj => match cat {            // char to right is (GAZ|EBG)
                    gr::GC_ZWJ => FindExtend, // rule GB11: ZWJ x (GAZ|EBG)
                    _ => {
                        take_curr = false;
                        break;
                    }
                },
                Unknown => unreachable!(),
            }
        }

        self.catb = if take_curr {
            None
        } else  {
            idx = previdx;
            Some(cat)
        };

        if self.extended && cat != gr::GC_Control {
            // rule GB9b: include any preceding Prepend characters
            for (i, c) in self.string[..idx].char_indices().rev() {
                match gr::grapheme_category(c) {
                    gr::GC_Prepend => idx = i,
                    cat => {
                        self.catb = Some(cat);
                        break;
                    }
                }
            }
        }

        let retstr = &self.string[idx..];
        self.string = &self.string[..idx];
        Some(retstr)
    }
}

#[inline]
pub fn new_graphemes<'b>(s: &'b str, is_extended: bool) -> Graphemes<'b> {
    Graphemes {
        string: s,
        extended: is_extended,
        cat: None,
        catb: None,
        regional_count_back: None
    }
}

#[inline]
pub fn new_grapheme_indices<'b>(s: &'b str, is_extended: bool) -> GraphemeIndices<'b> {
    GraphemeIndices { start_offset: s.as_ptr() as usize, iter: new_graphemes(s, is_extended) }
}

// maybe unify with PairResult?
#[derive(PartialEq, Eq)]
enum GraphemeCursorState {
    Unknown,
    NotBreak,
    Break,
    CheckCrlf,
    Regional,
    Emoji,
}

pub struct GraphemeCursor {
    offset: usize,  // current cursor position
    len: usize,  // total length of the string
    is_extended: bool,
    state: GraphemeCursorState,
    cat: Option<GraphemeCat>,  // category of codepoint immediately preceding cursor
    catb: Option<GraphemeCat>,  // category of codepoint immediately after cursor
    pre_context_offset: Option<usize>,
    ris_count: Option<usize>,
}

#[derive(PartialEq, Eq)]
pub enum GraphemeIncomplete {
    PreContext(usize),  // need pre-context for chunk ending at usize
    PrevChunk,  // requesting chunk previous to the one given
    NextChunk,  // requesting chunk following the one given
    InvalidOffset,  // error, chunk given is not inside cursor
}

#[derive(PartialEq, Eq)]
enum PairResult {
    NotBreak,  // definitely not a break
    Break,  // definitely a break
    Extended,  // a break if in extended mode
    CheckCrlf,  // a break unless it's a CR LF pair
    Regional,  // a break if preceded by an even number of RIS
    Emoji,  // a break if preceded by emoji base and extend
}

fn check_pair(before: GraphemeCat, after: GraphemeCat) -> PairResult {
    use tables::grapheme::GraphemeCat::*;
    use self::PairResult::*;
    match (before, after) {
        (GC_Control, GC_Control) => CheckCrlf,  // GB3
        (GC_Control, _) => Break,  // GB4
        (_, GC_Control) => Break,  // GB5
        (GC_L, GC_L) => NotBreak,  // GB6
        (GC_L, GC_V) => NotBreak,  // GB6
        (GC_L, GC_LV) => NotBreak,  // GB6
        (GC_L, GC_LVT) => NotBreak,  // GB6
        (GC_LV, GC_V) => NotBreak,  // GB7
        (GC_LV, GC_T) => NotBreak,  // GB7
        (GC_V, GC_V) => NotBreak,  // GB7
        (GC_V, GC_T) => NotBreak,  // GB7
        (GC_LVT, GC_T) => NotBreak,  // GB8
        (GC_T, GC_T) => NotBreak,  // GB8
        (_, GC_Extend) => NotBreak, // GB9
        (_, GC_ZWJ) => NotBreak,  // GB9
        (_, GC_SpacingMark) => Extended,  // GB9a
        (GC_Prepend, _) => Extended,  // GB9a
        (GC_E_Base, GC_E_Modifier) => NotBreak,  // GB10
        (GC_E_Base_GAZ, GC_E_Modifier) => NotBreak,  // GB10
        (GC_Extend, GC_E_Modifier) => Emoji,  // GB10
        (GC_ZWJ, GC_Glue_After_Zwj) => NotBreak,  // GB11
        (GC_Regional_Indicator, GC_Regional_Indicator) => Regional,  // GB12, GB13
        (_, _) => Break,  // GB999
    }
}

impl GraphemeCursor {
    pub fn new(offset: usize, len: usize, is_extended: bool) -> GraphemeCursor {
        use tables::grapheme as gr;
        let state = if offset == 0 || offset == len {
            GraphemeCursorState::Break
        } else {
            GraphemeCursorState::Unknown
        };
        GraphemeCursor {
            offset: offset,
            len: len,
            state: state,
            is_extended: is_extended,
            cat: None,
            catb: None,
            pre_context_offset: None,
            ris_count: None,
        }
    }

    pub fn set_cursor(&mut self, offset: usize) {
        if offset != self.offset {
            self.offset = offset;
            self.state = if offset == 0 || offset == self.len {
                GraphemeCursorState::Break
            } else {
                GraphemeCursorState::Unknown
            };
        }
    }

    pub fn provide_context(&mut self, chunk: &str, chunk_start: usize) {
        use tables::grapheme as gr;
        assert!(chunk_start + chunk.len() == self.pre_context_offset.unwrap());
        self.pre_context_offset = None;
        if self.is_extended && chunk_start + chunk.len() == self.offset {
            let ch = chunk.chars().rev().next().unwrap();
            if gr::grapheme_category(ch) == gr::GC_Prepend {
                self.decide(false);  // GB9b
                return;
            }
        }
        match self.state {
            GraphemeCursorState::CheckCrlf => {
                let is_break = chunk.as_bytes()[chunk.len() - 1] != b'\r';
                self.decide(is_break);
            }
            GraphemeCursorState::Regional => self.handle_regional(chunk, chunk_start),
            GraphemeCursorState::Emoji => self.handle_emoji(chunk, chunk_start),
            _ => panic!("invalid state")
        }
    }

    fn decide(&mut self, is_break: bool) {
        self.state = if is_break {
            GraphemeCursorState::Break
        } else {
            GraphemeCursorState::NotBreak
        };
    }

    fn decision(&mut self, is_break: bool) -> Result<bool, GraphemeIncomplete> {
        self.decide(is_break);
        Ok(is_break)
    }

    fn is_boundary_result(&self) -> Result<bool, GraphemeIncomplete> {
        if self.state == GraphemeCursorState::Break {
            Ok(true)
        } else if self.state == GraphemeCursorState::NotBreak {
            Ok(false)
        } else if let Some(pre_context_offset) = self.pre_context_offset {
            Err(GraphemeIncomplete::PreContext(pre_context_offset))
        } else {
            unreachable!("inconsistent state");
        }
    }

    fn handle_regional(&mut self, chunk: &str, chunk_start: usize) {
        use tables::grapheme as gr;
        let mut ris_count = self.ris_count.unwrap_or(0);
        for ch in chunk.chars().rev() {
            if gr::grapheme_category(ch) != gr::GC_Regional_Indicator {
                self.ris_count = Some(ris_count);
                self.decide((ris_count & 1) == 0);
                return;
            }
            ris_count += 1;
        }
        self.ris_count = Some(ris_count);
        if chunk_start == 0 {
            self.decide((ris_count & 1) == 0);
            return;
        }
        self.pre_context_offset = Some(chunk_start);
    }

    fn handle_emoji(&mut self, chunk: &str, chunk_start: usize) {
        use tables::grapheme as gr;
        for ch in chunk.chars().rev() {
            match gr::grapheme_category(ch) {
                gr::GC_Extend => (),
                gr::GC_E_Base | gr::GC_E_Base_GAZ => {
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
            return;
        }
        self.pre_context_offset = Some(chunk_start);
    }

    pub fn is_boundary(&mut self, chunk: &str, chunk_start: usize) -> Result<bool, GraphemeIncomplete> {
        use tables::grapheme as gr;
        if self.state == GraphemeCursorState::Break {
            return Ok(true)
        }
        if self.state == GraphemeCursorState::NotBreak {
            return Ok(false)
        }
        if self.offset < chunk_start || self.offset >= chunk_start + chunk.len() {
            return Err(GraphemeIncomplete::InvalidOffset)
        }
        if let Some(pre_context_offset) = self.pre_context_offset {
            return Err(GraphemeIncomplete::PreContext(pre_context_offset));
        }
        let offset_in_chunk = self.offset - chunk_start;
        if self.catb.is_none() {
            let ch = chunk[offset_in_chunk..].chars().next().unwrap();
            self.catb = Some(gr::grapheme_category(ch));
        }
        if self.offset == chunk_start {
            match self.catb.unwrap() {
                gr::GC_Control => {
                    if chunk.as_bytes()[offset_in_chunk] == b'\n' {
                        self.state = GraphemeCursorState::CheckCrlf;
                    }
                }
                gr::GC_Regional_Indicator => self.state = GraphemeCursorState::Regional,
                gr::GC_E_Modifier => self.state = GraphemeCursorState::Emoji,
                _ => ()
            }
            self.pre_context_offset = Some(chunk_start);
            return Err(GraphemeIncomplete::PreContext(chunk_start));
        }
        if self.cat.is_none() {
            let ch = chunk[..offset_in_chunk].chars().rev().next().unwrap();
            self.cat = Some(gr::grapheme_category(ch));
        }
        match check_pair(self.cat.unwrap(), self.catb.unwrap()) {
            PairResult::NotBreak => return self.decision(false),
            PairResult::Break => return self.decision(true),
            PairResult::Extended => {
                let is_extended = self.is_extended;
                return self.decision(is_extended);
            }
            PairResult::CheckCrlf => {
                if chunk.as_bytes()[offset_in_chunk] != b'\n' {
                    return self.decision(true);
                }
                if self.offset > chunk_start {
                    return self.decision(chunk.as_bytes()[offset_in_chunk - 1] != b'\r');
                }
                self.state = GraphemeCursorState::CheckCrlf;
                return Err(GraphemeIncomplete::PreContext(chunk_start));
            }
            PairResult::Regional => {
                self.handle_regional(&chunk[..offset_in_chunk], chunk_start);
                self.is_boundary_result()
            }
            PairResult::Emoji => {
                self.handle_emoji(&chunk[..offset_in_chunk], chunk_start);
                self.is_boundary_result()
            }
        }
    }

    pub fn next_boundary(&mut self, chunk: &str, chunk_start: usize) -> Result<Option<usize>, GraphemeIncomplete> {
        if self.offset == self.len {
            return Ok(None);
        }
        loop {
            let ch = chunk[self.offset - chunk_start..].chars().next().unwrap();
            self.offset += ch.len_utf8();
            self.cat = self.catb.take();
            self.state = GraphemeCursorState::Unknown;
            if let (Some(ris_count), Some(cat)) = (self.ris_count, self.cat) {
                if cat == GraphemeCat::GC_Regional_Indicator {
                    self.ris_count = Some(ris_count + 1);
                } else {
                    self.ris_count = Some(0);
                }
            }
            if self.offset == self.len {
                self.decide(true);
            } else if self.offset >= chunk_start + chunk.len() {
                return Err(GraphemeIncomplete::NextChunk);
            }
            if self.is_boundary(chunk, chunk_start)? {
                return Ok(Some(self.offset));
            }
        }
    }

    pub fn prev_boundary(&mut self, chunk: &str, chunk_start: usize) -> Result<Option<usize>, GraphemeIncomplete> {
        if self.offset == 0 {
            return Ok(None);
        }
        loop {
            if self.offset == chunk_start {
                return Err(GraphemeIncomplete::PrevChunk);
            }
            let ch = chunk[..self.offset - chunk_start].chars().rev().next().unwrap();
            self.offset -= ch.len_utf8();
            self.catb = self.cat.take();
            self.state = GraphemeCursorState::Unknown;
            if let Some(ris_count) = self.ris_count {
                self.ris_count = if ris_count > 0 { Some(ris_count - 1) } else { None };
            }
            if self.offset == 0 {
                self.decide(true);
            }
            if self.is_boundary(chunk, chunk_start)? {
                return Ok(Some(self.offset));
            }
        }
    }
}
