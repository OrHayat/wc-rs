#[cfg(test)]
#[cfg(target_arch = "aarch64")]
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
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::C) };
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
                let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), LocaleEncoding::C) };
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
}
