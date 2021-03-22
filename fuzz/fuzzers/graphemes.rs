#![no_main]


extern crate libfuzzer_sys;


extern crate unicode_segmentation;

use unicode_segmentation::UnicodeSegmentation;
use std::str;


#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    if let Ok(s) = str::from_utf8(data) {
		let g = UnicodeSegmentation::graphemes(s, true).collect::<Vec<_>>();
		assert!(UnicodeSegmentation::graphemes(s, true).rev().eq(g.iter().rev().cloned()));
	}
}
