#![no_main]


extern crate libfuzzer_sys;


extern crate unicode_segmentation;

use unicode_segmentation::UnicodeSegmentation;
use std::str;


#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    if let Ok(s) = str::from_utf8(data) {
        let g = s.split_word_bounds().collect::<Vec<_>>();
        assert!(s.split_word_bounds().rev().eq(g.iter().rev().cloned()));
    }
}
