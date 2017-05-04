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

// All of the logic for forward iteration over sentences
mod fwd {
    use tables::sentence::SentenceCat;
    use core::cmp;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum StatePart {
        Sot,
        Eot,
        Other,
        CR,
        LF,
        Sep,
        ATerm,
        UpperLower,
        ClosePlus,
        SpPlus,
        STerm
    }

    #[derive(Clone, PartialEq, Eq)]
    struct SentenceBreaksState(pub [StatePart; 4]);

    const INITIAL_STATE: SentenceBreaksState = SentenceBreaksState([
        StatePart::Sot,
        StatePart::Sot,
        StatePart::Sot,
        StatePart::Sot
    ]);

    pub struct SentenceBreaks<'a> {
        pub string: &'a str,
        pos: usize,
        state: SentenceBreaksState
    }

    impl SentenceBreaksState {
        fn next(&self, cat: SentenceCat) -> SentenceBreaksState {
            let &SentenceBreaksState(parts) = self;
            let parts = match (parts[3], cat) {
                (StatePart::ClosePlus, SentenceCat::SC_Close) => parts,
                (StatePart::SpPlus, SentenceCat::SC_Sp) => parts,
                _ => [
                    parts[1],
                    parts[2],
                    parts[3],
                    match cat {
                        SentenceCat::SC_CR => StatePart::CR,
                        SentenceCat::SC_LF => StatePart::LF,
                        SentenceCat::SC_Sep => StatePart::Sep,
                        SentenceCat::SC_ATerm => StatePart::ATerm,
                        SentenceCat::SC_Upper |
                        SentenceCat::SC_Lower => StatePart::UpperLower,
                        SentenceCat::SC_Close => StatePart::ClosePlus,
                        SentenceCat::SC_Sp => StatePart::SpPlus,
                        SentenceCat::SC_STerm => StatePart::STerm,
                        _ => StatePart::Other
                    }
                ]
            };
            SentenceBreaksState(parts)
        }

        fn end(&self) -> SentenceBreaksState {
            let &SentenceBreaksState(parts) = self;
            SentenceBreaksState([
                parts[1],
                parts[2],
                parts[3],
                StatePart::Eot
            ])
        }

        fn match1(&self, part: StatePart) -> bool {
            let &SentenceBreaksState(parts) = self;
            part == parts[3]
        }

        fn match2(&self, part1: StatePart, part2: StatePart) -> bool {
            let &SentenceBreaksState(parts) = self;
            part1 == parts[2] && part2 == parts[3]
        }
    }

    fn match_sb8(state: &SentenceBreaksState, ahead: &str) -> bool {
        let aterm_part = {
            // ATerm Close* Sp*
            let &SentenceBreaksState(parts) = state;
            let mut idx = if parts[3] == StatePart::SpPlus { 2 } else { 3 };
            if parts[idx] == StatePart::ClosePlus { idx -= 1 }
            parts[idx]
        };

        if aterm_part == StatePart::ATerm {
            use tables::sentence as se;

            for next_char in ahead.chars() {
                //( ¬(OLetter | Upper | Lower | ParaSep | SATerm) )* Lower
                match se::sentence_category(next_char) {
                    se::SC_Lower => return true,
                    se::SC_OLetter |
                    se::SC_Upper |
                    se::SC_Sep | se::SC_CR | se::SC_LF |
                    se::SC_STerm | se::SC_ATerm => return false,
                    _ => continue
                }
            }
        }

        false
    }

    fn match_sb8a(state: &SentenceBreaksState) -> bool {
        // SATerm Close* Sp*
        let &SentenceBreaksState(parts) = state;
        let mut idx = if parts[3] == StatePart::SpPlus { 2 } else { 3 };
        if parts[idx] == StatePart::ClosePlus { idx -= 1 }
        parts[idx] == StatePart::STerm || parts[idx] == StatePart::ATerm
    }

    fn match_sb9(state: &SentenceBreaksState) -> bool {
        // SATerm Close*
        let &SentenceBreaksState(parts) = state;
        let idx = if parts[3] == StatePart::ClosePlus { 2 } else { 3 };
        parts[idx] == StatePart::STerm || parts[idx] == StatePart::ATerm
    }

    fn match_sb11(state: &SentenceBreaksState) -> bool {
        // SATerm Close* Sp* ParaSep?
        let &SentenceBreaksState(parts) = state;
        let mut idx = match parts[3] {
            StatePart::Sep |
            StatePart::CR |
            StatePart::LF => 2,
            _ => 3
        };

        if parts[idx] == StatePart::SpPlus { idx -= 1 }
        if parts[idx] == StatePart::ClosePlus { idx -= 1}

        parts[idx] == StatePart::STerm || parts[idx] == StatePart::ATerm
    }

    impl<'a> Iterator for SentenceBreaks<'a> {
        // Returns the index of the character which follows a break
        type Item = usize;

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            let slen = self.string.len();
            // A sentence could be one character
            (cmp::min(slen, 2), Some(slen + 1))
        }

        #[inline]
        fn next(&mut self) -> Option<usize> {
            use tables::sentence as se;

            for next_char in self.string[self.pos..].chars() {
                let position_before = self.pos;
                let state_before = self.state.clone();

                let next_cat = se::sentence_category(next_char);

                self.pos += next_char.len_utf8();
                self.state = self.state.next(next_cat);

                match next_cat {
                    // SB1
                    _ if state_before.match1(StatePart::Sot) =>
                        return Some(position_before),

                    // SB3
                    SentenceCat::SC_LF if state_before.match1(StatePart::CR) =>
                        continue,

                    // SB4
                    _ if state_before.match1(StatePart::Sep)
                        || state_before.match1(StatePart::CR)
                        || state_before.match1(StatePart::LF)
                    => return Some(position_before),

                    // SB5
                    SentenceCat::SC_Extend |
                    SentenceCat::SC_Format => self.state = state_before,

                    // SB6
                    SentenceCat::SC_Numeric if state_before.match1(StatePart::ATerm) =>
                        continue,

                    // SB7
                    SentenceCat::SC_Upper if state_before.match2(StatePart::UpperLower, StatePart::ATerm) =>
                        continue,

                    // SB8
                    _ if match_sb8(&state_before, &self.string[position_before..]) =>
                        continue,

                    // SB8a
                    SentenceCat::SC_SContinue |
                    SentenceCat::SC_STerm |
                    SentenceCat::SC_ATerm if match_sb8a(&state_before) =>
                        continue,

                    // SB9
                    SentenceCat::SC_Close |
                    SentenceCat::SC_Sp |
                    SentenceCat::SC_Sep |
                    SentenceCat::SC_CR |
                    SentenceCat::SC_LF if match_sb9(&state_before) =>
                        continue,

                    // SB10
                    SentenceCat::SC_Sp |
                    SentenceCat::SC_Sep |
                    SentenceCat::SC_CR |
                    SentenceCat::SC_LF if match_sb8a(&state_before) =>
                        continue,

                    // SB11
                    _ if match_sb11(&state_before) =>
                        return Some(position_before),

                    // SB998
                    _ => continue
                }
            }

            // SB2
            if self.state.match1(StatePart::Sot) {
                None
            } else if self.state.match1(StatePart::Eot) {
                None
            } else {
                self.state = self.state.end();
                Some(self.pos)
            }
        }
    }

    pub fn new_sentence_breaks<'a>(source: &'a str) -> SentenceBreaks<'a> {
        SentenceBreaks { string: source, pos: 0, state: INITIAL_STATE }
    }

}

/// External iterator for a string's
/// [sentence boundaries](http://www.unicode.org/reports/tr29/#Sentence_Boundaries).
pub struct USentenceBounds<'a> {
    iter: fwd::SentenceBreaks<'a>,
    sentence_start: Option<usize>
}

#[inline]
pub fn new_sentence_bounds<'a>(source: &'a str) -> USentenceBounds<'a> {
    USentenceBounds {
        iter: fwd::new_sentence_breaks(source),
        sentence_start: None
    }
}

impl<'a> Iterator for USentenceBounds<'a> {
    type Item = &'a str;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        (cmp::max(0, lower - 1), upper.map(|u| cmp::max(0, u - 1)))
    }

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        if self.sentence_start == None {
            if let Some(start_pos) = self.iter.next() {
                self.sentence_start = Some(start_pos)
            } else {
                return None
            }
        }

        if let Some(break_pos) = self.iter.next() {
            let start_pos = self.sentence_start.unwrap();
            let sentence = &self.iter.string[start_pos..break_pos];
            self.sentence_start = Some(break_pos);
            Some(sentence)
        } else {
            None
        }
    }
}
