#![no_main]

use libfuzzer_sys::fuzz_target;
use wc_rs::{CountingBackend, LocaleEncoding};

fuzz_target!(|data: &[u8]| {
    // Test word counting with arbitrary byte sequences in both locale modes
    // This will catch panics, overflows, and logic errors

    // Test UTF-8 mode
    let _ = CountingBackend::Scalar.count_text(data, LocaleEncoding::Utf8);

    // Test SingleByte mode
    let _ = CountingBackend::Scalar.count_text(data, LocaleEncoding::SingleByte);

    // Both should complete without panicking
});
