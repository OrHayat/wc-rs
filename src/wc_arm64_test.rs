#[cfg(test)]
#[cfg(target_arch = "aarch64")]
#[cfg(sve_available)]
mod tests {
    use crate::wc_default_test::tests::common_word_count_cases;
    use crate::wc_default_test::tests::counts;
    use crate::{FileCounts, LocaleEncoding};
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use rstest::rstest;
    use rstest_reuse;
    use rstest_reuse::*;

    // Apply the common template to test NEON implementation
    // This will run all common test cases with the NEON implementation directly (not SVE!)
    #[apply(common_word_count_cases)]
    fn test_word_count_neon(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        // Call count_text_neon directly to ensure we're testing NEON, not SVE
        let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), locale) };
        assert_eq!(result, expected);
    }

    // SVE-specific test - only runs if SVE is available
    #[apply(common_word_count_cases)]
    fn test_word_count_sve(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        // Runtime check for SVE support
        if !std::arch::is_aarch64_feature_detected!("sve") {
            // Rust doesn't have built-in test skip - early return is idiomatic
            // Print for visibility during test runs
            eprintln!("SKIP: SVE not available on this CPU");
            return;
        }

        let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), locale) };
        assert_eq!(result, expected);
    }

    // ====================================================================
    // Property-Based Tests (PropTest)
    // ====================================================================

    // Property: bytes == input length (ARM64 NEON)
    proptest! {
        #[test]
        fn prop_bytes_equals_input_length_neon(input in "\\PC*") {
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };
            prop_assert_eq!(result.bytes, input.len(), "NEON");
        }
    }

    // Property: bytes == input length (ARM64 SVE)
    proptest! {
        #[test]
        fn prop_bytes_equals_input_length_sve(input in "\\PC*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, input.len(), "SVE");
            }
        }
    }

    // Property 2: bytes >= chars >= lines
    proptest! {
        #[test]
        fn prop_bytes_ge_chars_ge_lines_neon(input in "\\PC*") {
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };
            prop_assert!(result.bytes >= result.chars,
                "NEON: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
            prop_assert!(result.chars >= result.lines,
                "NEON: chars ({}) must be >= lines ({})", result.chars, result.lines);
        }
    }

    proptest! {
        #[test]
        fn prop_bytes_ge_chars_ge_lines_sve(input in "\\PC*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "SVE: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
                prop_assert!(result.chars >= result.lines,
                    "SVE: chars ({}) must be >= lines ({})", result.chars, result.lines);
            }
        }
    }

    // Property 3: C locale - lines <= chars == bytes (every byte is a char)
    proptest! {
        #[test]
        fn prop_c_locale_lines_le_chars_eq_bytes_neon(input in "\\PC*") {
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::SingleByte) };
            prop_assert_eq!(result.chars, result.bytes,
                "NEON C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
            prop_assert!(result.lines <= result.chars,
                "NEON C locale: lines ({}) must be <= chars ({})", result.lines, result.chars);
        }
    }

    proptest! {
        #[test]
        fn prop_c_locale_lines_le_chars_eq_bytes_sve(input in "\\PC*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::SingleByte) };
                prop_assert_eq!(result.chars, result.bytes,
                    "SVE C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
                prop_assert!(result.lines <= result.chars,
                    "SVE C locale: lines ({}) must be <= chars ({})", result.lines, result.chars);
            }
        }
    }

    // Property 4a: Line counting accuracy - no newlines
    proptest! {
        #[test]
        fn prop_lines_zero_no_newlines_neon(input in "\\PC*") {
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };
            prop_assert_eq!(result.lines, 0,
                "NEON no newlines: lines must be 0, got {}", result.lines);
        }
    }

    proptest! {
        #[test]
        fn prop_lines_zero_no_newlines_sve(input in "\\PC*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.lines, 0,
                    "SVE no newlines: lines must be 0, got {}", result.lines);
            }
        }
    }

    // Property 4b: Line counting accuracy - with newlines
    proptest! {
        #[test]
        fn prop_lines_count_accurate_neon(input in ".*") {
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };
            let expected_lines = input.chars().filter(|&c| c == '\n').count();
            prop_assert_eq!(result.lines, expected_lines,
                "NEON: lines ({}) must equal newline count ({})", result.lines, expected_lines);
        }
    }

    proptest! {
        #[test]
        fn prop_lines_count_accurate_sve(input in ".*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };
                let expected_lines = input.chars().filter(|&c| c == '\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "SVE: lines ({}) must equal newline count ({})", result.lines, expected_lines);
            }
        }
    }

    // Property 5: ASCII fast path - all bytes < 0x80 means bytes == chars
    proptest! {
        #[test]
        fn prop_ascii_bytes_eq_chars_neon(input in "[\\x00-\\x7F]*") {
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };
            prop_assert_eq!(result.bytes, result.chars,
                "NEON ASCII: bytes ({}) must equal chars ({})", result.bytes, result.chars);
        }
    }

    proptest! {
        #[test]
        fn prop_ascii_bytes_eq_chars_sve(input in "[\\x00-\\x7F]*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, result.chars,
                    "SVE ASCII: bytes ({}) must equal chars ({})", result.bytes, result.chars);
            }
        }
    }

    // Property 6: All whitespace → words == 0 (and verify other counts)
    proptest! {
        #[test]
        fn prop_whitespace_zero_words_neon(input in "\\s*") {
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };
            prop_assert_eq!(result.words, 0,
                "NEON all whitespace: words must be 0, got {}", result.words);
            prop_assert_eq!(result.bytes, input.len(),
                "NEON all whitespace: bytes ({}) must equal input length ({})", result.bytes, input.len());
            prop_assert_eq!(result.chars, input.chars().count(),
                "NEON all whitespace: chars ({}) must equal char count ({})", result.chars, input.chars().count());
            let expected_lines = input.chars().filter(|&c| c == '\n').count();
            prop_assert_eq!(result.lines, expected_lines,
                "NEON all whitespace: lines ({}) must equal newline count ({})", result.lines, expected_lines);
        }
    }

    proptest! {
        #[test]
        fn prop_whitespace_zero_words_sve(input in "\\s*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.words, 0,
                    "SVE all whitespace: words must be 0, got {}", result.words);
                prop_assert_eq!(result.bytes, input.len(),
                    "SVE all whitespace: bytes ({}) must equal input length ({})", result.bytes, input.len());
                prop_assert_eq!(result.chars, input.chars().count(),
                    "SVE all whitespace: chars ({}) must equal char count ({})", result.chars, input.chars().count());
                let expected_lines = input.chars().filter(|&c| c == '\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "SVE all whitespace: lines ({}) must equal newline count ({})", result.lines, expected_lines);
            }
        }
    }

    // Property 7: Differential Testing - SIMD == Scalar (ASCII inputs, UTF-8 locale)
    proptest! {
        #[test]
        fn prop_differential_neon_vs_scalar_utf8_ascii(input in "[\\x00-\\x7F]*") {
            let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            let simd = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };

            prop_assert_eq!(scalar.lines, simd.lines, "NEON: lines mismatch");
            prop_assert_eq!(scalar.words, simd.words, "NEON: words mismatch");
            prop_assert_eq!(scalar.bytes, simd.bytes, "NEON: bytes mismatch");
            prop_assert_eq!(scalar.chars, simd.chars, "NEON: chars mismatch");
        }
    }

    proptest! {
        #[test]
        fn prop_differential_sve_vs_scalar_utf8_ascii(input in "[\\x00-\\x7F]*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "SVE: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SVE: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SVE: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SVE: chars mismatch");
            }
        }
    }

    // Property 7b: Differential Testing - SIMD == Scalar (Printable Unicode, UTF-8 locale)
    proptest! {
        #[test]
        fn prop_differential_neon_vs_scalar_utf8_unicode(input in "\\PC*") {
            let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            let simd = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };

            prop_assert_eq!(scalar.lines, simd.lines, "NEON Unicode: lines mismatch");
            prop_assert_eq!(scalar.words, simd.words, "NEON Unicode: words mismatch");
            prop_assert_eq!(scalar.bytes, simd.bytes, "NEON Unicode: bytes mismatch");
            prop_assert_eq!(scalar.chars, simd.chars, "NEON Unicode: chars mismatch");
        }
    }

    proptest! {
        #[test]
        fn prop_differential_sve_vs_scalar_utf8_unicode(input in "\\PC*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "SVE Unicode: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SVE Unicode: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SVE Unicode: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SVE Unicode: chars mismatch");
            }
        }
    }

    // Property 7c: Differential Testing - SIMD == Scalar (All chars including control, UTF-8 locale)
    proptest! {
        #[test]
        fn prop_differential_neon_vs_scalar_utf8_all(input in ".*") {
            let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            let simd = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };

            prop_assert_eq!(scalar.lines, simd.lines, "NEON all: lines mismatch");
            prop_assert_eq!(scalar.words, simd.words, "NEON all: words mismatch");
            prop_assert_eq!(scalar.bytes, simd.bytes, "NEON all: bytes mismatch");
            prop_assert_eq!(scalar.chars, simd.chars, "NEON all: chars mismatch");
        }
    }

    proptest! {
        #[test]
        fn prop_differential_sve_vs_scalar_utf8_all(input in ".*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "SVE all: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SVE all: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SVE all: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SVE all: chars mismatch");
            }
        }
    }

    // Property 7d: Differential Testing - SIMD == Scalar (All chars, C locale)
    proptest! {
        #[test]
        fn prop_differential_neon_vs_scalar_c_all(input in ".*") {
            let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::SingleByte);
            let simd = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::SingleByte) };

            prop_assert_eq!(scalar.lines, simd.lines, "NEON C: lines mismatch");
            prop_assert_eq!(scalar.words, simd.words, "NEON C: words mismatch");
            prop_assert_eq!(scalar.bytes, simd.bytes, "NEON C: bytes mismatch");
            prop_assert_eq!(scalar.chars, simd.chars, "NEON C: chars mismatch");
        }
    }

    proptest! {
        #[test]
        fn prop_differential_sve_vs_scalar_c_all(input in ".*") {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::SingleByte);
                let simd = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::SingleByte) };

                prop_assert_eq!(scalar.lines, simd.lines, "SVE C: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SVE C: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SVE C: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SVE C: chars mismatch");
            }
        }
    }

    // ====================================================================
    // Invalid UTF-8 Property Tests (SIMD)
    // ====================================================================
    use proptest::collection::vec as prop_vec;

    // Property: Invalid UTF-8 → bytes >= chars (NEON)
    proptest! {
        #[test]
        fn prop_invalid_utf8_bytes_ge_chars_neon(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            let result = unsafe { crate::wc_arm64::count_text_neon(&invalid_bytes, LocaleEncoding::Utf8) };
            prop_assert!(result.bytes >= result.chars,
                "NEON Invalid UTF-8: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
        }
    }

    // Property: Invalid UTF-8 → bytes >= chars (SVE)
    proptest! {
        #[test]
        fn prop_invalid_utf8_bytes_ge_chars_sve(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(&invalid_bytes, LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "SVE Invalid UTF-8: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
            }
        }
    }

    // Property: Differential - Invalid UTF-8 (NEON == Scalar)
    proptest! {
        #[test]
        fn prop_differential_neon_vs_scalar_invalid_utf8(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            let scalar = crate::wc_default::word_count_scalar(&invalid_bytes, LocaleEncoding::Utf8);
            let simd = unsafe { crate::wc_arm64::count_text_neon(&invalid_bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(scalar.lines, simd.lines, "NEON invalid UTF-8: lines mismatch");
            prop_assert_eq!(scalar.words, simd.words, "NEON invalid UTF-8: words mismatch");
            prop_assert_eq!(scalar.bytes, simd.bytes, "NEON invalid UTF-8: bytes mismatch");
            prop_assert_eq!(scalar.chars, simd.chars, "NEON invalid UTF-8: chars mismatch");
        }
    }

    // Property: Differential - Invalid UTF-8 (SVE == Scalar)
    proptest! {
        #[test]
        fn prop_differential_sve_vs_scalar_invalid_utf8(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let scalar = crate::wc_default::word_count_scalar(&invalid_bytes, LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_arm64::count_text_sve(&invalid_bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "SVE invalid UTF-8: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SVE invalid UTF-8: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SVE invalid UTF-8: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SVE invalid UTF-8: chars mismatch");
            }
        }
    }

    // Property: C locale with any bytes (NEON)
    proptest! {
        #[test]
        fn prop_c_locale_any_bytes_neon(bytes in prop_vec(0u8..=255u8, 0..100)) {
            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::SingleByte) };
            prop_assert_eq!(result.chars, result.bytes,
                "NEON C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
        }
    }

    // Property: C locale with any bytes (SVE)
    proptest! {
        #[test]
        fn prop_c_locale_any_bytes_sve(bytes in prop_vec(0u8..=255u8, 0..100)) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::SingleByte) };
                prop_assert_eq!(result.chars, result.bytes,
                    "SVE C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
            }
        }
    }

    // ====================================================================
    // Additional Advanced Property Tests (matching wc_default_test.rs)
    // ====================================================================

    // Property: Invalid UTF-8 bytes join words (NEON)
    proptest! {
        #[test]
        fn prop_invalid_utf8_joins_words_neon(
            prefix in "[a-z]+",
            invalid_byte in 0x80u8..=0xFFu8,
            suffix in "[a-z]+"
        ) {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(prefix.as_bytes());
            bytes.push(invalid_byte);
            bytes.extend_from_slice(suffix.as_bytes());

            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.words, 1,
                "NEON: Invalid UTF-8 byte 0x{:02X} should join words, got {} words", invalid_byte, result.words);

            let expected_chars = prefix.chars().count() + suffix.chars().count();
            prop_assert_eq!(result.chars, expected_chars,
                "NEON: Invalid UTF-8: chars should be {} (prefix + suffix), got {}", expected_chars, result.chars);
        }
    }

    proptest! {
        #[test]
        fn prop_invalid_utf8_joins_words_sve(
            prefix in "[a-z]+",
            invalid_byte in 0x80u8..=0xFFu8,
            suffix in "[a-z]+"
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(prefix.as_bytes());
                bytes.push(invalid_byte);
                bytes.extend_from_slice(suffix.as_bytes());

                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.words, 1,
                    "SVE: Invalid UTF-8 byte 0x{:02X} should join words, got {} words", invalid_byte, result.words);

                let expected_chars = prefix.chars().count() + suffix.chars().count();
                prop_assert_eq!(result.chars, expected_chars,
                    "SVE: Invalid UTF-8: chars should be {} (prefix + suffix), got {}", expected_chars, result.chars);
            }
        }
    }

    // Property: Lone continuation bytes don't form words (NEON)
    proptest! {
        #[test]
        fn prop_lone_continuation_bytes_neon(
            continuation_byte in 0x80u8..=0xBFu8,
            count in 1usize..10
        ) {
            let bytes = vec![continuation_byte; count];
            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.words, 0,
                "NEON: Lone continuation bytes should not form words, got {}", result.words);
            prop_assert_eq!(result.chars, 0,
                "NEON: Lone continuation bytes should not count as chars, got {}", result.chars);
            prop_assert_eq!(result.bytes, count);
        }
    }

    proptest! {
        #[test]
        fn prop_lone_continuation_bytes_sve(
            continuation_byte in 0x80u8..=0xBFu8,
            count in 1usize..10
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let bytes = vec![continuation_byte; count];
                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.words, 0,
                    "SVE: Lone continuation bytes should not form words, got {}", result.words);
                prop_assert_eq!(result.chars, 0,
                    "SVE: Lone continuation bytes should not count as chars, got {}", result.chars);
                prop_assert_eq!(result.bytes, count);
            }
        }
    }

    // Property: Truncated UTF-8 sequences at end (NEON)
    proptest! {
        #[test]
        fn prop_truncated_sequences_at_end_neon(
            prefix in "[a-z]{0,20}",
            start_byte in prop::sample::select(vec![
                0xC2u8, 0xC3u8,  // 2-byte starts
                0xE0u8, 0xE1u8,  // 3-byte starts
                0xF0u8, 0xF1u8,  // 4-byte starts
            ])
        ) {
            let mut bytes = Vec::from(prefix.as_bytes());
            bytes.push(start_byte);

            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.bytes, bytes.len());

            let expected_chars = prefix.chars().count();
            prop_assert_eq!(result.chars, expected_chars,
                "NEON: Chars should be {} (prefix only), got {}", expected_chars, result.chars);
        }
    }

    proptest! {
        #[test]
        fn prop_truncated_sequences_at_end_sve(
            prefix in "[a-z]{0,20}",
            start_byte in prop::sample::select(vec![
                0xC2u8, 0xC3u8,  // 2-byte starts
                0xE0u8, 0xE1u8,  // 3-byte starts
                0xF0u8, 0xF1u8,  // 4-byte starts
            ])
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let mut bytes = Vec::from(prefix.as_bytes());
                bytes.push(start_byte);

                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.bytes, bytes.len());

                let expected_chars = prefix.chars().count();
                prop_assert_eq!(result.chars, expected_chars,
                    "SVE: Chars should be {} (prefix only), got {}", expected_chars, result.chars);
            }
        }
    }

    // Property: Invalid bytes don't increment line count (NEON)
    proptest! {
        #[test]
        fn prop_invalid_bytes_no_lines_neon(
            invalid_bytes in prop_vec(0x80u8..=0xFFu8, 1..50)
        ) {
            let result = unsafe { crate::wc_arm64::count_text_neon(&invalid_bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.lines, 0,
                "NEON: Invalid bytes without \\n should have 0 lines, got {}", result.lines);
        }
    }

    proptest! {
        #[test]
        fn prop_invalid_bytes_no_lines_sve(
            invalid_bytes in prop_vec(0x80u8..=0xFFu8, 1..50)
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(&invalid_bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.lines, 0,
                    "SVE: Invalid bytes without \\n should have 0 lines, got {}", result.lines);
            }
        }
    }

    // Property: Mix of valid ASCII and invalid bytes (NEON)
    proptest! {
        #[test]
        fn prop_mixed_ascii_invalid_neon(
            valid in "[a-z]{1,20}",
            invalid_count in 1usize..5
        ) {
            let mut bytes = Vec::from(valid.as_bytes());
            for _ in 0..invalid_count {
                bytes.push(0xFF);
            }

            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.words, 1,
                "NEON: Valid + invalid should form 1 word, got {}", result.words);

            prop_assert_eq!(result.chars, valid.len(),
                "NEON: Chars should be {} (valid only), got {}", valid.len(), result.chars);

            prop_assert_eq!(result.bytes, valid.len() + invalid_count);
        }
    }

    proptest! {
        #[test]
        fn prop_mixed_ascii_invalid_sve(
            valid in "[a-z]{1,20}",
            invalid_count in 1usize..5
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let mut bytes = Vec::from(valid.as_bytes());
                for _ in 0..invalid_count {
                    bytes.push(0xFF);
                }

                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.words, 1,
                    "SVE: Valid + invalid should form 1 word, got {}", result.words);

                prop_assert_eq!(result.chars, valid.len(),
                    "SVE: Chars should be {} (valid only), got {}", valid.len(), result.chars);

                prop_assert_eq!(result.bytes, valid.len() + invalid_count);
            }
        }
    }

    // Property: Invalid bytes with newlines (NEON)
    proptest! {
        #[test]
        fn prop_invalid_with_newlines_neon(
            lines in prop::collection::vec("[a-z]{0,10}", 1..10)
        ) {
            let mut bytes = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                bytes.extend_from_slice(line.as_bytes());
                bytes.push(0xFF);
                if i < lines.len() - 1 {
                    bytes.push(b'\n');
                }
            }

            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            let expected_lines = lines.len() - 1;
            prop_assert_eq!(result.lines, expected_lines,
                "NEON: Lines should be {}, got {}", expected_lines, result.lines);

            let total_ascii: usize = lines.iter().map(|s| s.len()).sum();
            let expected_chars = total_ascii + expected_lines;
            prop_assert_eq!(result.chars, expected_chars,
                "NEON: Chars should be {}, got {}", expected_chars, result.chars);
        }
    }

    proptest! {
        #[test]
        fn prop_invalid_with_newlines_sve(
            lines in prop::collection::vec("[a-z]{0,10}", 1..10)
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let mut bytes = Vec::new();
                for (i, line) in lines.iter().enumerate() {
                    bytes.extend_from_slice(line.as_bytes());
                    bytes.push(0xFF);
                    if i < lines.len() - 1 {
                        bytes.push(b'\n');
                    }
                }

                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                let expected_lines = lines.len() - 1;
                prop_assert_eq!(result.lines, expected_lines,
                    "SVE: Lines should be {}, got {}", expected_lines, result.lines);

                let total_ascii: usize = lines.iter().map(|s| s.len()).sum();
                let expected_chars = total_ascii + expected_lines;
                prop_assert_eq!(result.chars, expected_chars,
                    "SVE: Chars should be {}, got {}", expected_chars, result.chars);
            }
        }
    }

    // Property: High invalid start bytes (0xF5-0xFF) (NEON)
    proptest! {
        #[test]
        fn prop_high_invalid_start_bytes_neon(
            invalid_byte in 0xF5u8..=0xFFu8,
            count in 1usize..10
        ) {
            let bytes = vec![invalid_byte; count];
            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.chars, 0,
                "NEON: High invalid bytes should not count as chars, got {}", result.chars);
            prop_assert_eq!(result.words, 0,
                "NEON: Isolated invalid bytes should not form words, got {}", result.words);
            prop_assert_eq!(result.bytes, count);
        }
    }

    proptest! {
        #[test]
        fn prop_high_invalid_start_bytes_sve(
            invalid_byte in 0xF5u8..=0xFFu8,
            count in 1usize..10
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let bytes = vec![invalid_byte; count];
                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.chars, 0,
                    "SVE: High invalid bytes should not count as chars, got {}", result.chars);
                prop_assert_eq!(result.words, 0,
                    "SVE: Isolated invalid bytes should not form words, got {}", result.words);
                prop_assert_eq!(result.bytes, count);
            }
        }
    }

    // Property: Overlong encodings are invalid (NEON)
    proptest! {
        #[test]
        fn prop_overlong_encodings_invalid_neon(
            ascii_char in 0x00u8..=0x7Fu8
        ) {
            let overlong = vec![
                0xC0 | (ascii_char >> 6),
                0x80 | (ascii_char & 0x3F),
            ];

            let result = unsafe { crate::wc_arm64::count_text_neon(&overlong, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.chars, 0,
                "NEON: Overlong encoding should not count as char, got {}", result.chars);
            prop_assert_eq!(result.bytes, 2);
        }
    }

    proptest! {
        #[test]
        fn prop_overlong_encodings_invalid_sve(
            ascii_char in 0x00u8..=0x7Fu8
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let overlong = vec![
                    0xC0 | (ascii_char >> 6),
                    0x80 | (ascii_char & 0x3F),
                ];

                let result = unsafe { crate::wc_arm64::count_text_sve(&overlong, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.chars, 0,
                    "SVE: Overlong encoding should not count as char, got {}", result.chars);
                prop_assert_eq!(result.bytes, 2);
            }
        }
    }

    // Property: Random byte sequences - fundamental invariants (NEON)
    proptest! {
        #[test]
        fn prop_random_bytes_invariant_neon(
            bytes in prop_vec(0u8..=255u8, 1..200)
        ) {
            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            prop_assert!(result.bytes >= result.chars,
                "NEON: bytes ({}) must be >= chars ({})", result.bytes, result.chars);

            prop_assert_eq!(result.bytes, bytes.len(),
                "NEON: bytes must equal input length: {} != {}", result.bytes, bytes.len());

            prop_assert!(result.lines <= result.chars,
                "NEON: lines ({}) must be <= chars ({})", result.lines, result.chars);
        }
    }

    proptest! {
        #[test]
        fn prop_random_bytes_invariant_sve(
            bytes in prop_vec(0u8..=255u8, 1..200)
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                prop_assert!(result.bytes >= result.chars,
                    "SVE: bytes ({}) must be >= chars ({})", result.bytes, result.chars);

                prop_assert_eq!(result.bytes, bytes.len(),
                    "SVE: bytes must equal input length: {} != {}", result.bytes, bytes.len());

                prop_assert!(result.lines <= result.chars,
                    "SVE: lines ({}) must be <= chars ({})", result.lines, result.chars);
            }
        }
    }

    // Property: Invalid bytes at SIMD boundaries (NEON)
    proptest! {
        #[test]
        fn prop_invalid_at_simd_boundaries_neon(
            boundary_size in prop::sample::select(vec![16usize, 32, 64]),
            invalid_byte in 0x80u8..=0xFFu8
        ) {
            let mut bytes = vec![b'a'; boundary_size - 1];
            bytes.push(invalid_byte);

            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.words, 1,
                "NEON: Should form 1 word at boundary {}, got {}", boundary_size, result.words);

            prop_assert_eq!(result.chars, boundary_size - 1,
                "NEON: Chars should be {}, got {}", boundary_size - 1, result.chars);

            prop_assert_eq!(result.bytes, boundary_size);
        }
    }

    proptest! {
        #[test]
        fn prop_invalid_at_simd_boundaries_sve(
            boundary_size in prop::sample::select(vec![16usize, 32, 64]),
            invalid_byte in 0x80u8..=0xFFu8
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let mut bytes = vec![b'a'; boundary_size - 1];
                bytes.push(invalid_byte);

                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.words, 1,
                    "SVE: Should form 1 word at boundary {}, got {}", boundary_size, result.words);

                prop_assert_eq!(result.chars, boundary_size - 1,
                    "SVE: Chars should be {}, got {}", boundary_size - 1, result.chars);

                prop_assert_eq!(result.bytes, boundary_size);
            }
        }
    }

    // Property: Multiple invalid bytes between words (NEON)
    proptest! {
        #[test]
        fn prop_multiple_invalid_between_words_neon(
            word1 in "[a-z]{1,10}",
            word2 in "[a-z]{1,10}",
            invalid_count in 1usize..10
        ) {
            let mut bytes = Vec::from(word1.as_bytes());
            for _ in 0..invalid_count {
                bytes.push(0xFF);
            }
            bytes.extend_from_slice(word2.as_bytes());

            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::Utf8) };

            prop_assert_eq!(result.words, 1,
                "NEON: Multiple invalid bytes should join words, got {}", result.words);

            let expected_chars = word1.len() + word2.len();
            prop_assert_eq!(result.chars, expected_chars,
                "NEON: Chars should be {}, got {}", expected_chars, result.chars);
        }
    }

    proptest! {
        #[test]
        fn prop_multiple_invalid_between_words_sve(
            word1 in "[a-z]{1,10}",
            word2 in "[a-z]{1,10}",
            invalid_count in 1usize..10
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let mut bytes = Vec::from(word1.as_bytes());
                for _ in 0..invalid_count {
                    bytes.push(0xFF);
                }
                bytes.extend_from_slice(word2.as_bytes());

                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(result.words, 1,
                    "SVE: Multiple invalid bytes should join words, got {}", result.words);

                let expected_chars = word1.len() + word2.len();
                prop_assert_eq!(result.chars, expected_chars,
                    "SVE: Chars should be {}, got {}", expected_chars, result.chars);
            }
        }
    }

    // Property: C locale comprehensive (NEON)
    proptest! {
        #[test]
        fn prop_c_locale_comprehensive_neon(
            bytes in prop_vec(0u8..=255u8, 1..200)
        ) {
            let result = unsafe { crate::wc_arm64::count_text_neon(&bytes, LocaleEncoding::SingleByte) };

            prop_assert_eq!(result.chars, result.bytes,
                "NEON C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);

            let expected_lines = bytes.iter().filter(|&&b| b == b'\n').count();
            prop_assert_eq!(result.lines, expected_lines,
                "NEON C locale: lines ({}) must equal \\n count ({})", result.lines, expected_lines);
        }
    }

    proptest! {
        #[test]
        fn prop_c_locale_comprehensive_sve(
            bytes in prop_vec(0u8..=255u8, 1..200)
        ) {
            if std::arch::is_aarch64_feature_detected!("sve") {
                let result = unsafe { crate::wc_arm64::count_text_sve(&bytes, LocaleEncoding::SingleByte) };

                prop_assert_eq!(result.chars, result.bytes,
                    "SVE C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);

                let expected_lines = bytes.iter().filter(|&&b| b == b'\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "SVE C locale: lines ({}) must equal \\n count ({})", result.lines, expected_lines);
            }
        }
    }

    // ====================================================================
    // UTF-8 Chunk Boundary Tests
    // ====================================================================

    /// Helper: build content with UTF-8 char at specific byte offset
    fn build_boundary_content(padding: usize, utf8_char: &str) -> Vec<u8> {
        let mut content = vec![b'a'; padding];
        content.extend_from_slice(utf8_char.as_bytes());
        content
    }

    /// Test UTF-8 at SVE chunk boundaries (Rust wrapper - should pass)
    #[rstest]
    #[case::three_byte_at_254(254, "中", "3-byte at 254")]
    #[case::four_byte_at_253(253, "𐍈", "4-byte at 253")]
    #[case::two_byte_at_255(255, "é", "2-byte at 255")]
    #[case::three_byte_at_510(510, "中", "3-byte at 510")]
    fn test_utf8_chunk_boundary_sve_wrapper(
        #[case] padding: usize,
        #[case] utf8_char: &str,
        #[case] desc: &str,
    ) {
        if !std::arch::is_aarch64_feature_detected!("sve") {
            eprintln!("SKIP: SVE not available");
            return;
        }

        let content = build_boundary_content(padding, utf8_char);
        let result = unsafe { crate::wc_arm64::count_text_sve(&content, LocaleEncoding::Utf8) };
        let expected = crate::wc_default::word_count_scalar(&content, LocaleEncoding::Utf8);

        assert_eq!(result, expected, "{}: wrapper should match scalar", desc);
    }

    /// Direct C FFI test - exposes boundary bug
    mod sve_ffi_test {
        use crate::FileCounts;
        unsafe extern "C" {
            pub fn count_text_sve_c_unchecked(
                content: *const u8,
                len: usize,
                locale: u8,
            ) -> FileCounts;
        }
    }

    /// Test UTF-8 at chunk boundaries (C direct - EXPOSES BUG)
    #[rstest]
    #[case::three_byte_at_254(254, "中", "3-byte at 254")]
    #[case::four_byte_at_253(253, "𐍈", "4-byte at 253")]
    fn test_utf8_chunk_boundary_c_direct(
        #[case] padding: usize,
        #[case] utf8_char: &str,
        #[case] desc: &str,
    ) {
        if !std::arch::is_aarch64_feature_detected!("sve") {
            eprintln!("SKIP: SVE not available");
            return;
        }

        let content = build_boundary_content(padding, utf8_char);
        let result = unsafe {
            sve_ffi_test::count_text_sve_c_unchecked(content.as_ptr(), content.len(), 1)
        };
        let expected = crate::wc_default::word_count_scalar(&content, LocaleEncoding::Utf8);

        assert_eq!(result.chars, expected.chars,
            "{}: C direct chars mismatch (BUG: skips boundary chars)", desc);
    }
}
