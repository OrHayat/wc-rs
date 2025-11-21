#![no_main]

use libfuzzer_sys::fuzz_target;
use wc_rs::{CountingBackend, LocaleEncoding};

fuzz_target!(|data: &[u8]| {
    // Test counter overflow scenarios
    // Focus on inputs that might cause counters to overflow or wrap

    if data.is_empty() {
        return;
    }

    // Test with the input as-is
    let result = unsafe { CountingBackend::new_scalar_unchecked() }.count_text(data, LocaleEncoding::Utf8);

    // Basic invariants that should hold even near overflow
    assert!(result.chars <= result.bytes, "chars cannot exceed bytes");
    assert!(result.lines <= result.bytes, "lines cannot exceed bytes");
    assert!(result.words <= result.bytes, "words cannot exceed bytes");

    // Generate challenging overflow scenarios
    let mut test_cases = Vec::new();

    // 1. Maximum number of lines (every byte is a newline)
    test_cases.push(vec![b'\n'; data.len().min(10000)]);

    // 2. Maximum number of words (alternating word char and space)
    let mut max_words = Vec::new();
    for i in 0..data.len().min(10000) {
        max_words.push(if i % 2 == 0 { b'a' } else { b' ' });
    }
    test_cases.push(max_words);

    // 3. Maximum single-byte characters (ASCII)
    test_cases.push(vec![b'a'; data.len().min(10000)]);

    // 4. Multi-byte UTF-8 characters (fewer chars than bytes)
    // U+1F4A9 (💩) is 4 bytes in UTF-8
    let mut utf8_multibyte = Vec::new();
    for _ in 0..data.len().min(2500) {
        utf8_multibyte.extend_from_slice("💩".as_bytes());
    }
    test_cases.push(utf8_multibyte);

    // 5. Mix of everything to stress all counters at once
    let mut mixed = Vec::new();
    for i in 0..data.len().min(5000) {
        match i % 5 {
            0 => mixed.push(b'\n'),
            1 => mixed.push(b' '),
            2 => mixed.push(b'x'),
            3 => mixed.extend_from_slice("世".as_bytes()), // 3-byte UTF-8
            _ => mixed.extend_from_slice("🌍".as_bytes()), // 4-byte UTF-8
        }
    }
    test_cases.push(mixed);

    // 6. Very long lines (no newlines)
    test_cases.push(vec![b'a'; data.len().min(100000)]);

    // 7. Edge case: single very long word
    let long_word = data.iter()
        .filter(|&&b| b != b' ' && b != b'\n' && b != b'\t' && b != b'\r')
        .take(50000)
        .copied()
        .collect::<Vec<_>>();
    if !long_word.is_empty() {
        test_cases.push(long_word);
    }

    // Test all scenarios
    for test_case in &test_cases {
        let utf8_result = unsafe { CountingBackend::new_scalar_unchecked() }.count_text(test_case, LocaleEncoding::Utf8);
        let sb_result = unsafe { CountingBackend::new_scalar_unchecked() }.count_text(test_case, LocaleEncoding::SingleByte);

        // Verify invariants hold
        assert!(utf8_result.chars <= utf8_result.bytes, "UTF-8: chars > bytes");
        assert!(utf8_result.lines <= utf8_result.bytes, "UTF-8: lines > bytes");
        assert!(utf8_result.words <= utf8_result.bytes, "UTF-8: words > bytes");

        assert!(sb_result.chars <= sb_result.bytes, "SingleByte: chars > bytes");
        assert!(sb_result.lines <= sb_result.bytes, "SingleByte: lines > bytes");
        assert!(sb_result.words <= sb_result.bytes, "SingleByte: words > bytes");

        // bytes count should always equal input length
        assert_eq!(utf8_result.bytes, test_case.len(), "UTF-8: bytes mismatch");
        assert_eq!(sb_result.bytes, test_case.len(), "SingleByte: bytes mismatch");

        // In SingleByte mode, chars should equal bytes
        assert_eq!(sb_result.chars, sb_result.bytes, "SingleByte: chars != bytes");

        // No counter should overflow (they're usize, so we can't easily test true overflow,
        // but we can verify the math doesn't panic or produce nonsensical results)

        // Test that counters can be added without panicking
        let _ = utf8_result.lines.checked_add(utf8_result.words);
        let _ = utf8_result.bytes.checked_add(utf8_result.chars);
        let _ = sb_result.lines.checked_add(sb_result.words);
    }

    // Test with detected SIMD backend to ensure overflow handling is consistent
    let backend = CountingBackend::detect();
    let simd_result = backend.count_text(data, LocaleEncoding::Utf8);

    // SIMD result should match scalar
    let scalar_result = unsafe { CountingBackend::new_scalar_unchecked() }.count_text(data, LocaleEncoding::Utf8);
    assert_eq!(simd_result, scalar_result, "SIMD/scalar mismatch on overflow test");
});

