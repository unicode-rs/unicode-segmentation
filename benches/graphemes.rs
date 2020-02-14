#[macro_use]
extern crate bencher;
extern crate unicode_segmentation;

use bencher::Bencher;
use unicode_segmentation::UnicodeSegmentation;

const TEXT_ARABIC: &str = include_str!("texts/arabic.txt");
const TEXT_ENGLISH: &str = include_str!("texts/english.txt");
const TEXT_HINDI: &str = include_str!("texts/hindi.txt");
const TEXT_JAPANESE: &str = include_str!("texts/japanese.txt");
const TEXT_KOREAN: &str = include_str!("texts/korean.txt");
const TEXT_MANDARIN: &str = include_str!("texts/mandarin.txt");
const TEXT_RUSSIAN: &str = include_str!("texts/russian.txt");
const TEXT_SOURCE_CODE: &str = include_str!("texts/source_code.txt");

fn graphemes_arabic(bench: &mut Bencher) {
    bench.iter(|| {
        for g in UnicodeSegmentation::graphemes(TEXT_ARABIC, true) {
            bencher::black_box(g);
        }
    });

    bench.bytes = TEXT_ARABIC.len() as u64;
}

fn graphemes_english(bench: &mut Bencher) {
    bench.iter(|| {
        for g in UnicodeSegmentation::graphemes(TEXT_ENGLISH, true) {
            bencher::black_box(g);
        }
    });

    bench.bytes = TEXT_ENGLISH.len() as u64;
}

fn graphemes_hindi(bench: &mut Bencher) {
    bench.iter(|| {
        for g in UnicodeSegmentation::graphemes(TEXT_HINDI, true) {
            bencher::black_box(g);
        }
    });

    bench.bytes = TEXT_HINDI.len() as u64;
}

fn graphemes_japanese(bench: &mut Bencher) {
    bench.iter(|| {
        for g in UnicodeSegmentation::graphemes(TEXT_JAPANESE, true) {
            bencher::black_box(g);
        }
    });

    bench.bytes = TEXT_JAPANESE.len() as u64;
}

fn graphemes_korean(bench: &mut Bencher) {
    bench.iter(|| {
        for g in UnicodeSegmentation::graphemes(TEXT_KOREAN, true) {
            bencher::black_box(g);
        }
    });

    bench.bytes = TEXT_KOREAN.len() as u64;
}

fn graphemes_mandarin(bench: &mut Bencher) {
    bench.iter(|| {
        for g in UnicodeSegmentation::graphemes(TEXT_MANDARIN, true) {
            bencher::black_box(g);
        }
    });

    bench.bytes = TEXT_MANDARIN.len() as u64;
}

fn graphemes_russian(bench: &mut Bencher) {
    bench.iter(|| {
        for g in UnicodeSegmentation::graphemes(TEXT_RUSSIAN, true) {
            bencher::black_box(g);
        }
    });

    bench.bytes = TEXT_RUSSIAN.len() as u64;
}

fn graphemes_source_code(bench: &mut Bencher) {
    bench.iter(|| {
        for g in UnicodeSegmentation::graphemes(TEXT_SOURCE_CODE, true) {
            bencher::black_box(g);
        }
    });

    bench.bytes = TEXT_SOURCE_CODE.len() as u64;
}

benchmark_group!(
    benches,
    graphemes_arabic,
    graphemes_english,
    graphemes_hindi,
    graphemes_japanese,
    graphemes_korean,
    graphemes_mandarin,
    graphemes_russian,
    graphemes_source_code,
);

benchmark_main!(benches);
