// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::UnicodeSegmentation;

use std::prelude::v1::*;

#[test]
fn test_graphemes() {
    use testdata::{TEST_SAME, TEST_DIFF};

    pub const EXTRA_DIFF: &'static [(&'static str,
                                     &'static [&'static str],
                                     &'static [&'static str])] = &[
        // Official test suite doesn't include two Prepend chars between two other chars.
        ("\u{20}\u{600}\u{600}\u{20}",
         &["\u{20}", "\u{600}\u{600}\u{20}"],
         &["\u{20}", "\u{600}", "\u{600}", "\u{20}"]),

        // Test for Prepend followed by two Any chars
        ("\u{600}\u{20}\u{20}",
         &["\u{600}\u{20}", "\u{20}"],
         &["\u{600}", "\u{20}", "\u{20}"]),
    ];

    for &(s, g) in TEST_SAME {
        // test forward iterator
        assert!(UnicodeSegmentation::graphemes(s, true)
                .zip(g.iter().cloned())
                .all(|(a,b)| a == b));
        assert!(UnicodeSegmentation::graphemes(s, false)
                .zip(g.iter().cloned())
                .all(|(a,b)| a == b));

        // test reverse iterator
        assert!(UnicodeSegmentation::graphemes(s, true).rev()
                .zip(g.iter().rev().cloned())
                .all(|(a,b)| a == b));
        assert!(UnicodeSegmentation::graphemes(s, false).rev()
                .zip(g.iter().rev().cloned())
                .all(|(a,b)| a == b));
    }

    for &(s, gt, gf) in TEST_DIFF.iter().chain(EXTRA_DIFF) {
        // test forward iterator
        assert!(UnicodeSegmentation::graphemes(s, true)
                .zip(gt.iter().cloned())
                .all(|(a,b)| a == b), "{:?}", s);
        assert!(UnicodeSegmentation::graphemes(s, false)
                .zip(gf.iter().cloned())
                .all(|(a,b)| a == b));

        // test reverse iterator
        assert!(UnicodeSegmentation::graphemes(s, true).rev()
                .zip(gt.iter().rev().cloned())
                .all(|(a,b)| a == b));
        assert!(UnicodeSegmentation::graphemes(s, false).rev()
                .zip(gf.iter().rev().cloned())
                .all(|(a,b)| a == b));
    }

    // test the indices iterators
    let s = "a̐éö̲\r\n";
    let gr_inds = UnicodeSegmentation::grapheme_indices(s, true).collect::<Vec<(usize, &str)>>();
    let b: &[_] = &[(0, "a̐"), (3, "é"), (6, "ö̲"), (11, "\r\n")];
    assert_eq!(gr_inds, b);
    let gr_inds = UnicodeSegmentation::grapheme_indices(s, true).rev().collect::<Vec<(usize, &str)>>();
    let b: &[_] = &[(11, "\r\n"), (6, "ö̲"), (3, "é"), (0, "a̐")];
    assert_eq!(gr_inds, b);
    let mut gr_inds_iter = UnicodeSegmentation::grapheme_indices(s, true);
    {
        let gr_inds = gr_inds_iter.by_ref();
        let e1 = gr_inds.size_hint();
        assert_eq!(e1, (1, Some(13)));
        let c = gr_inds.count();
        assert_eq!(c, 4);
    }
    let e2 = gr_inds_iter.size_hint();
    assert_eq!(e2, (0, Some(0)));

    // make sure the reverse iterator does the right thing with "\n" at beginning of string
    let s = "\n\r\n\r";
    let gr = UnicodeSegmentation::graphemes(s, true).rev().collect::<Vec<&str>>();
    let b: &[_] = &["\r", "\r\n", "\n"];
    assert_eq!(gr, b);
}

#[test]
fn test_words() {
    use testdata::TEST_WORD;

    // Unicode's official tests don't really test longer chains of flag emoji
    // TODO This could be improved with more tests like flag emoji with interspersed Extend chars and ZWJ
    const EXTRA_TESTS: &'static [(&'static str, &'static [&'static str])] = &[
        ("🇦🇫🇦🇽🇦🇱🇩🇿🇦🇸🇦🇩🇦🇴", &["🇦🇫", "🇦🇽", "🇦🇱", "🇩🇿", "🇦🇸", "🇦🇩", "🇦🇴"]),
        ("🇦🇫🇦🇽🇦🇱🇩🇿🇦🇸🇦🇩🇦", &["🇦🇫", "🇦🇽", "🇦🇱", "🇩🇿", "🇦🇸", "🇦🇩", "🇦"]),
        ("🇦a🇫🇦🇽a🇦🇱🇩🇿🇦🇸🇦🇩🇦", &["🇦", "a", "🇫🇦", "🇽", "a", "🇦🇱", "🇩🇿", "🇦🇸", "🇦🇩", "🇦"]),
        ("\u{1f468}\u{200d}\u{1f468}\u{200d}\u{1f466}",  &["\u{1f468}\u{200d}\u{1f468}\u{200d}\u{1f466}"]),
        ("😌👎🏼",  &["😌", "👎🏼"]),
        // perhaps wrong, spaces should not be included?
        ("hello world", &["hello", " ", "world"]),
        ("🇨🇦🇨🇭🇿🇲🇿 hi", &["🇨🇦", "🇨🇭", "🇿🇲", "🇿", " ", "hi"]),
    ];
    for &(s, w) in TEST_WORD.iter().chain(EXTRA_TESTS.iter()) {
        macro_rules! assert_ {
            ($test:expr, $exp:expr, $name:expr) => {
                // collect into vector for better diagnostics in failure case
                let testing = $test.collect::<Vec<_>>();
                let expected = $exp.collect::<Vec<_>>();
                assert_eq!(testing, expected, "{} test for testcase ({:?}, {:?}) failed.", $name, s, w)
            }
        }
        // test forward iterator
        assert_!(s.split_word_bounds(),
                w.iter().cloned(),
                "Forward word boundaries");

        // test reverse iterator
        assert_!(s.split_word_bounds().rev(),
                w.iter().rev().cloned(),
                "Reverse word boundaries");

        // generate offsets from word string lengths
        let mut indices = vec![0];
        for i in w.iter().cloned().map(|s| s.len()).scan(0, |t, n| { *t += n; Some(*t) }) {
            indices.push(i);
        }
        indices.pop();
        let indices = indices;

        // test forward indices iterator
        assert_!(s.split_word_bound_indices().map(|(l,_)| l),
                 indices.iter().cloned(),
                 "Forward word indices");

        // test backward indices iterator
        assert_!(s.split_word_bound_indices().rev().map(|(l,_)| l),
                 indices.iter().rev().cloned(),
                 "Reverse word indices");
    }
}
