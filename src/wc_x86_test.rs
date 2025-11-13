#[cfg(test)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod tests {
    use crate::wc_default_test::tests::{common_word_count_cases, counts};
    use crate::wc_x86::count_text_simd;
    use crate::{FileCounts, LocaleEncoding};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use rstest_reuse;
    use rstest_reuse::*;

    // Apply the common template to test NEON implementation
    // This will run all 42 common test cases with the NEON implementation
    #[apply(common_word_count_cases)]
    fn test_count_text_simd(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        let result = count_text_simd(input, locale).expect("NEON should be available on aarch64");
        assert_eq!(result, expected);
    }

    // NEON-specific test cases can be added here
    // These test edge cases specific to SIMD implementation

    // TODO: Add NEON-specific edge cases:
    // - Chunk boundary cases (16-byte boundaries)
    // - Very long strings (stress test SIMD loop)
    // - Strings that trigger scalar fallback paths
}
