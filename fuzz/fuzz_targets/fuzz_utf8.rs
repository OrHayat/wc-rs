#![no_main]

use libfuzzer_sys::fuzz_target;
use wc_rs::{CountingBackend, LocaleEncoding};

fuzz_target!(|data: &[u8]| {
    // Test UTF-8 handling with various invalid and edge-case sequences
    // This specifically targets UTF-8 boundary detection and character counting

    // Run the UTF-8 word counter - should handle all invalid UTF-8 gracefully
    let result = unsafe { CountingBackend::new_scalar_unchecked() }.count_text(data, LocaleEncoding::Utf8);

    // Verify basic invariants
    assert!(result.chars <= result.bytes, "chars cannot exceed bytes");
    assert!(result.lines <= result.bytes, "lines cannot exceed bytes");

    // If data is valid UTF-8, chars should match std's count
    if let Ok(s) = std::str::from_utf8(data) {
        assert_eq!(result.chars, s.chars().count(),
            "valid UTF-8 char count should match std");
    }
});
