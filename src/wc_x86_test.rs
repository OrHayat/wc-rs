#[cfg(test)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod tests {
    use crate::wc_default_test::tests::{common_word_count_cases, counts};
    use crate::wc_x86::count_text_simd;
    use crate::{FileCounts, LocaleEncoding};
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
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
    #[apply(common_word_count_cases)]
    fn test_count_text_sse2_manual(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        if is_x86_feature_detected!("sse2") {
            let result = unsafe { crate::wc_x86::count_text_sse2_manual(input.as_bytes(), locale) };
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

    // ====================================================================
    // Property-Based Tests (PropTest)
    // ====================================================================

    // Property: bytes == input length
    proptest! {
        #[test]
        fn prop_bytes_equals_input_length_sse2(input in "\\PC*") {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, input.len(), "SSE2");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_bytes_equals_input_length_avx2(input in "\\PC*") {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, input.len(), "AVX2");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_bytes_equals_input_length_avx512(input in "\\PC*") {
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let result = unsafe { crate::wc_x86::count_text_avx512bw(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, input.len(), "AVX512BW");
            }
        }
    }

    // Property 2: bytes >= chars >= lines (x86 SSE2)
    proptest! {
        #[test]
        fn prop_bytes_ge_chars_ge_lines_sse2(input in "\\PC*") {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "SSE2: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
                prop_assert!(result.chars >= result.lines,
                    "SSE2: chars ({}) must be >= lines ({})", result.chars, result.lines);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_bytes_ge_chars_ge_lines_avx2(input in "\\PC*") {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "AVX2: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
                prop_assert!(result.chars >= result.lines,
                    "AVX2: chars ({}) must be >= lines ({})", result.chars, result.lines);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_bytes_ge_chars_ge_lines_avx512(input in "\\PC*") {
            if is_x86_feature_detected!("avx512bw") {
                let result = unsafe { crate::wc_x86::count_text_avx512bw(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "AVX512BW: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
                prop_assert!(result.chars >= result.lines,
                    "AVX512BW: chars ({}) must be >= lines ({})", result.chars, result.lines);
            }
        }
    }

    // Property 3: C locale - lines <= chars == bytes (every byte is a char)
    proptest! {
        #[test]
        fn prop_c_locale_lines_le_chars_eq_bytes_sse2(input in "\\PC*") {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::C) };
                prop_assert_eq!(result.chars, result.bytes,
                    "SSE2 C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
                prop_assert!(result.lines <= result.chars,
                    "SSE2 C locale: lines ({}) must be <= chars ({})", result.lines, result.chars);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_c_locale_lines_le_chars_eq_bytes_avx2(input in "\\PC*") {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::C) };
                prop_assert_eq!(result.chars, result.bytes,
                    "AVX2 C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
                prop_assert!(result.lines <= result.chars,
                    "AVX2 C locale: lines ({}) must be <= chars ({})", result.lines, result.chars);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_c_locale_lines_le_chars_eq_bytes_avx512(input in "\\PC*") {
            if is_x86_feature_detected!("avx512bw") {
                let result = unsafe { crate::wc_x86::count_text_avx512bw(input.as_bytes(), LocaleEncoding::C) };
                prop_assert_eq!(result.chars, result.bytes,
                    "AVX512BW C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
                prop_assert!(result.lines <= result.chars,
                    "AVX512BW C locale: lines ({}) must be <= chars ({})", result.lines, result.chars);
            }
        }
    }
}
