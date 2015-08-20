Iterators which split strings on Grapheme Cluster or Word boundaries, according
to the [Unicode Standard Annex #29](http://www.unicode.org/reports/tr29/) rules.

[![Build Status](https://travis-ci.org/unicode-rs/unicode-segmentation.svg)](https://travis-ci.org/unicode-rs/unicode-segmentation)

[Documentation](https://unicode-rs.github.io/unicode-segmentation/unicode_segmentation/index.html)

```rust
extern crate unicode_segmentation;

use unicode_segmentation::UnicodeSegmentation;

fn main() {
    let s = "a̐éö̲\r\n";
    let g = UnicodeSegmentation::graphemes(s, true).collect::<Vec<&str>>();
    let b: &[_] = &["a̐", "é", "ö̲", "\r\n"];
    assert_eq!(g, b);

    let s = "The quick (\"brown\") fox can't jump 32.3 feet, right?";
    let w = s.unicode_words().collect::<Vec<&str>>();
    let b: &[_] = &["The", "quick", "brown", "fox", "can't", "jump", "32.3", "feet", "right"];
    assert_eq!(w, b);

    let s = "The quick (\"brown\")  fox";
    let w = s.split_word_bounds().collect::<Vec<&str>>();
    let b: &[_] = &["The", " ", "quick", " ", "(", "\"", "brown", "\"", ")", " ", " ", "fox"];
    assert_eq!(w, b);
}
```

# features

unicode-segmentation supports a `no_std` feature. This eliminates dependence on std,
and instead uses equivalent functions from core.

# crates.io

You can use this package in your project by adding the following
to your `Cargo.toml`:

```toml
[dependencies]
unicode-segmentation = "0.1.2"
```
