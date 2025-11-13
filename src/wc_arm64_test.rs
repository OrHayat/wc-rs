#[cfg(test)]
#[cfg(target_arch = "aarch64")]
mod tests {
    use crate::wc_arm64::count_text_simd;
    use crate::wc_default_test::tests::common_word_count_cases;
    use crate::{FileCounts, LocaleEncoding};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use rstest_reuse;
    use rstest_reuse::*;

    // Re-export counts helper for template expansion
    use crate::wc_default_test::tests::counts;

    // Apply the common template to test NEON implementation
    // This will run all 42 common test cases with the NEON implementation
    #[apply(common_word_count_cases)]
    fn test_word_count_neon(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        let result = count_text_simd(input.as_bytes(), locale)
            .expect("NEON should be available on aarch64");
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

        // Test SVE implementation directly (not NEON fallback)
        let result = unsafe { crate::wc_arm64::count_text_sve(input.as_bytes(), locale) };
        assert_eq!(result, expected);
    }
}
