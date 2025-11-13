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

    // Apply the common template to test SIMD auto-detection
    // This will run all common test cases with automatic SIMD selection
    #[apply(common_word_count_cases)]
    fn test_count_text_simd(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        let result = count_text_simd(input, locale).expect("simd should be available on x86_64");
        assert_eq!(result, expected);
    }

    // Test SSE2 implementation specifically
    // SSE2 is available on all x86_64 CPUs, so we can test it directly
    #[apply(common_word_count_cases)]
    fn test_count_text_sse2(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        if is_x86_feature_detected!("sse2") {
            let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), locale) };
            assert_eq!(result, expected);
        }
    }

    // Test AVX2 implementation specifically
    // Only runs if AVX2 is available on the current CPU
    #[apply(common_word_count_cases)]
    fn test_count_text_avx2(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        if is_x86_feature_detected!("avx2") {
            let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), locale) };
            assert_eq!(result, expected);
        }
    }

    // Test AVX512BW implementation specifically
    // Only runs if AVX512BW is available on the current CPU
    #[apply(common_word_count_cases)]
    fn test_count_text_avx512bw(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        if is_x86_feature_detected!("avx512bw") {
            let result = unsafe { crate::wc_x86::count_text_avx512bw(input.as_bytes(), locale) };
            assert_eq!(result, expected);
        }
    }
}
