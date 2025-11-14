#[cfg(test)]
#[cfg(target_arch = "aarch64")]
mod tests {
    use crate::wc_default_test::tests::common_word_count_cases;
    use crate::{FileCounts, LocaleEncoding};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use rstest_reuse;
    use rstest_reuse::*;

    // Re-export counts helper for template expansion
    use crate::wc_default_test::tests::counts;

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
    use proptest::prelude::*;

    // Property: bytes == input length (ARM64 NEON)
    #[cfg(target_arch = "aarch64")]
    proptest! {
        #[test]
        fn prop_bytes_equals_input_length_neon(input in "\\PC*") {
            let result = unsafe { crate::wc_arm64::count_text_neon(input.as_bytes(), LocaleEncoding::Utf8) };
            prop_assert_eq!(result.bytes, input.len(), "NEON");
        }
    }

    // Property: bytes == input length (ARM64 SVE)
    #[cfg(target_arch = "aarch64")]
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
    #[cfg(target_arch = "aarch64")]
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

    #[cfg(target_arch = "aarch64")]
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
}
