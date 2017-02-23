#![no_main]

extern crate libfuzzer_sys;
extern crate unicode_segmentation;
use std::str;
use unicode_segmentation::UnicodeSegmentation;
#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    if let Ok(s) = str::from_utf8(data) {
        let result = UnicodeSegmentation::graphemes(s, true).flat_map(|s| s.chars()).collect::<String>();
        assert_eq!(s, result);
        let result = s.split_word_bounds().flat_map(|s| s.chars()).collect::<String>();
        assert_eq!(s, result);

    }
}
