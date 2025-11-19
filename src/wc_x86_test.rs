#[cfg(test)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod tests {
    use crate::wc_default_test::tests::{common_word_count_cases, counts};
    use crate::{FileCounts, LocaleEncoding};
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use proptest::collection::vec as prop_vec;
    use rstest::rstest;
    use rstest_reuse;
    use rstest_reuse::*;

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

    // Test AVX512 implementation specifically
    #[apply(common_word_count_cases)]
    fn test_count_text_avx512(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        if is_x86_feature_detected!("avx512bw") && is_x86_feature_detected!("avx512f") {
            let result = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), locale) };
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
                let result = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, input.len(), "AVX512");
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
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let result = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "AVX512: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
                prop_assert!(result.chars >= result.lines,
                    "AVX512: chars ({}) must be >= lines ({})", result.chars, result.lines);
            }
        }
    }

    // Property 3: C locale - lines <= chars == bytes (every byte is a char)
    proptest! {
        #[test]
        fn prop_c_locale_lines_le_chars_eq_bytes_sse2(input in "\\PC*") {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::SingleByte) };
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
                let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::SingleByte) };
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
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let result = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::SingleByte) };
                prop_assert_eq!(result.chars, result.bytes,
                    "AVX512 C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
                prop_assert!(result.lines <= result.chars,
                    "AVX512 C locale: lines ({}) must be <= chars ({})", result.lines, result.chars);
            }
        }
    }

    // Property 4a: Line counting accuracy - no newlines
    proptest! {
        #[test]
        fn prop_lines_zero_no_newlines_sse2(input in "\\PC*") {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.lines, 0,
                    "SSE2 no newlines: lines must be 0, got {}", result.lines);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_lines_zero_no_newlines_avx2(input in "\\PC*") {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.lines, 0,
                    "AVX2 no newlines: lines must be 0, got {}", result.lines);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_lines_zero_no_newlines_avx512(input in "\\PC*") {
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let result = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.lines, 0,
                    "AVX512 no newlines: lines must be 0, got {}", result.lines);
            }
        }
    }

    // Property 4b: Line counting accuracy - with newlines
    proptest! {
        #[test]
        fn prop_lines_count_accurate_sse2(input in ".*") {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };
                let expected_lines = input.chars().filter(|&c| c == '\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "SSE2: lines ({}) must equal newline count ({})", result.lines, expected_lines);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_lines_count_accurate_avx2(input in ".*") {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };
                let expected_lines = input.chars().filter(|&c| c == '\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "AVX2: lines ({}) must equal newline count ({})", result.lines, expected_lines);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_lines_count_accurate_avx512(input in ".*") {
            if is_x86_feature_detected!("avx512bw") {
                let result = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };
                let expected_lines = input.chars().filter(|&c| c == '\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "AVX512: lines ({}) must equal newline count ({})", result.lines, expected_lines);
            }
        }
    }

    // Property 5: ASCII fast path - all bytes < 0x80 means bytes == chars
    proptest! {
        #[test]
        fn prop_ascii_bytes_eq_chars_sse2(input in "[\\x00-\\x7F]*") {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, result.chars,
                    "SSE2 ASCII: bytes ({}) must equal chars ({})", result.bytes, result.chars);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_ascii_bytes_eq_chars_avx2(input in "[\\x00-\\x7F]*") {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, result.chars,
                    "AVX2 ASCII: bytes ({}) must equal chars ({})", result.bytes, result.chars);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_ascii_bytes_eq_chars_avx512(input in "[\\x00-\\x7F]*") {
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let result = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.bytes, result.chars,
                    "AVX512 ASCII: bytes ({}) must equal chars ({})", result.bytes, result.chars);
            }
        }
    }

    // Property 6: All whitespace → words == 0 (and verify other counts)
    proptest! {
        #[test]
        fn prop_whitespace_zero_words_sse2(input in "\\s*") {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.words, 0,
                    "SSE2 all whitespace: words must be 0, got {}", result.words);
                prop_assert_eq!(result.bytes, input.len(),
                    "SSE2 all whitespace: bytes ({}) must equal input length ({})", result.bytes, input.len());
                prop_assert_eq!(result.chars, input.chars().count(),
                    "SSE2 all whitespace: chars ({}) must equal char count ({})", result.chars, input.chars().count());
                let expected_lines = input.chars().filter(|&c| c == '\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "SSE2 all whitespace: lines ({}) must equal newline count ({})", result.lines, expected_lines);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_whitespace_zero_words_avx2(input in "\\s*") {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.words, 0,
                    "AVX2 all whitespace: words must be 0, got {}", result.words);
                prop_assert_eq!(result.bytes, input.len(),
                    "AVX2 all whitespace: bytes ({}) must equal input length ({})", result.bytes, input.len());
                prop_assert_eq!(result.chars, input.chars().count(),
                    "AVX2 all whitespace: chars ({}) must equal char count ({})", result.chars, input.chars().count());
                let expected_lines = input.chars().filter(|&c| c == '\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "AVX2 all whitespace: lines ({}) must equal newline count ({})", result.lines, expected_lines);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_whitespace_zero_words_avx512(input in "\\s*") {
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let result = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };
                prop_assert_eq!(result.words, 0,
                    "AVX512 all whitespace: words must be 0, got {}", result.words);
                prop_assert_eq!(result.bytes, input.len(),
                    "AVX512 all whitespace: bytes ({}) must equal input length ({})", result.bytes, input.len());
                prop_assert_eq!(result.chars, input.chars().count(),
                    "AVX512 all whitespace: chars ({}) must equal char count ({})", result.chars, input.chars().count());
                let expected_lines = input.chars().filter(|&c| c == '\n').count();
                prop_assert_eq!(result.lines, expected_lines,
                    "AVX512 all whitespace: lines ({}) must equal newline count ({})", result.lines, expected_lines);
            }
        }
    }

    // Property 7: Differential Testing - SIMD == Scalar (ASCII inputs, UTF-8 locale)
    proptest! {
        #[test]
        fn prop_differential_sse2_vs_scalar_utf8_ascii(input in "[\\x00-\\x7F]*") {
            if is_x86_feature_detected!("sse2") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "SSE2: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SSE2: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SSE2: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SSE2: chars mismatch");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_differential_avx2_vs_scalar_utf8_ascii(input in "[\\x00-\\x7F]*") {
            if is_x86_feature_detected!("avx2") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX2: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX2: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX2: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX2: chars mismatch");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_differential_avx512_vs_scalar_utf8_ascii(input in "[\\x00-\\x7F]*") {
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX512: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX512: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX512: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX512: chars mismatch");
            }
        }
    }

    // Property 7b: Differential Testing - SIMD == Scalar (Printable Unicode, UTF-8 locale)
    proptest! {
        #[test]
        fn prop_differential_sse2_vs_scalar_utf8_unicode(input in "\\PC*") {
            if is_x86_feature_detected!("sse2") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "SSE2 Unicode: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SSE2 Unicode: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SSE2 Unicode: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SSE2 Unicode: chars mismatch");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_differential_avx2_vs_scalar_utf8_unicode(input in "\\PC*") {
            if is_x86_feature_detected!("avx2") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX2 Unicode: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX2 Unicode: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX2 Unicode: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX2 Unicode: chars mismatch");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_differential_avx512_vs_scalar_utf8_unicode(input in "\\PC*") {
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX512 Unicode: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX512 Unicode: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX512 Unicode: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX512 Unicode: chars mismatch");
            }
        }
    }

    // Property 7c: Differential Testing - SIMD == Scalar (All chars including control, UTF-8 locale)
    proptest! {
        #[test]
        fn prop_differential_sse2_vs_scalar_utf8_all(input in ".*") {
            if is_x86_feature_detected!("sse2") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "SSE2 all: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SSE2 all: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SSE2 all: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SSE2 all: chars mismatch");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_differential_avx2_vs_scalar_utf8_all(input in ".*") {
            if is_x86_feature_detected!("avx2") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX2 all: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX2 all: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX2 all: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX2 all: chars mismatch");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_differential_avx512_vs_scalar_utf8_all(input in ".*") {
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX512 all: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX512 all: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX512 all: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX512 all: chars mismatch");
            }
        }
    }

    // Property 7d: Differential Testing - SIMD == Scalar (All chars, C locale)
    proptest! {
        #[test]
        fn prop_differential_sse2_vs_scalar_c_all(input in ".*") {
            if is_x86_feature_detected!("sse2") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::SingleByte);
                let simd = unsafe { crate::wc_x86::count_text_sse2(input.as_bytes(), LocaleEncoding::SingleByte) };

                prop_assert_eq!(scalar.lines, simd.lines, "SSE2 C: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SSE2 C: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SSE2 C: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SSE2 C: chars mismatch");
            }
        }
    }

    // ====================================================================
    // Invalid UTF-8 Property Tests (SIMD)
    // ====================================================================

    // Property: Invalid UTF-8 → bytes >= chars (SSE2)
    proptest! {
        #[test]
        fn prop_invalid_utf8_bytes_ge_chars_sse2(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(&invalid_bytes, LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "SSE2 Invalid UTF-8: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
            }
        }
    }

    // Property: Invalid UTF-8 → bytes >= chars (AVX2)
    proptest! {
        #[test]
        fn prop_invalid_utf8_bytes_ge_chars_avx2(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(&invalid_bytes, LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "AVX2 Invalid UTF-8: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
            }
        }
    }

    // Property: Invalid UTF-8 → bytes >= chars (AVX512)
    proptest! {
        #[test]
        fn prop_invalid_utf8_bytes_ge_chars_avx512(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("avx512bw") && is_x86_feature_detected!("avx512f") {
                let result = unsafe { crate::wc_x86::count_text_avx512(&invalid_bytes, LocaleEncoding::Utf8) };
                prop_assert!(result.bytes >= result.chars,
                    "AVX512 Invalid UTF-8: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
            }
        }
    }

    // Property: Differential - Invalid UTF-8 (SSE2 == Scalar)
    proptest! {
        #[test]
        fn prop_differential_sse2_vs_scalar_invalid_utf8(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("sse2") {
                let scalar = crate::wc_default::word_count_scalar(&invalid_bytes, LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_sse2(&invalid_bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "SSE2 invalid UTF-8: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "SSE2 invalid UTF-8: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "SSE2 invalid UTF-8: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "SSE2 invalid UTF-8: chars mismatch");
            }
        }
    }

    // Property: Differential - Invalid UTF-8 (AVX2 == Scalar)
    proptest! {
        #[test]
        fn prop_differential_avx2_vs_scalar_invalid_utf8(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("avx2") {
                let scalar = crate::wc_default::word_count_scalar(&invalid_bytes, LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_avx2(&invalid_bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX2 invalid UTF-8: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX2 invalid UTF-8: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX2 invalid UTF-8: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX2 invalid UTF-8: chars mismatch");
            }
        }
    }

    // Property: Differential - Invalid UTF-8 (AVX512 == Scalar)
    proptest! {
        #[test]
        fn prop_differential_avx512_vs_scalar_invalid_utf8(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("avx512bw") && is_x86_feature_detected!("avx512f") {
                let scalar = crate::wc_default::word_count_scalar(&invalid_bytes, LocaleEncoding::Utf8);
                let simd = unsafe { crate::wc_x86::count_text_avx512(&invalid_bytes, LocaleEncoding::Utf8) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX512 invalid UTF-8: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX512 invalid UTF-8: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX512 invalid UTF-8: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX512 invalid UTF-8: chars mismatch");
            }
        }
    }

    // Property: C locale with any bytes (SSE2)
    proptest! {
        #[test]
        fn prop_c_locale_any_bytes_sse2(bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("sse2") {
                let result = unsafe { crate::wc_x86::count_text_sse2(&bytes, LocaleEncoding::SingleByte) };
                prop_assert_eq!(result.chars, result.bytes,
                    "SSE2 C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
            }
        }
    }

    // Property: C locale with any bytes (AVX2)
    proptest! {
        #[test]
        fn prop_c_locale_any_bytes_avx2(bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("avx2") {
                let result = unsafe { crate::wc_x86::count_text_avx2(&bytes, LocaleEncoding::SingleByte) };
                prop_assert_eq!(result.chars, result.bytes,
                    "AVX2 C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
            }
        }
    }

    // Property: C locale with any bytes (AVX512)
    proptest! {
        #[test]
        fn prop_c_locale_any_bytes_avx512(bytes in prop_vec(0u8..=255u8, 0..100)) {
            if is_x86_feature_detected!("avx512bw") && is_x86_feature_detected!("avx512f") {
                let result = unsafe { crate::wc_x86::count_text_avx512(&bytes, LocaleEncoding::SingleByte) };
                prop_assert_eq!(result.chars, result.bytes,
                    "AVX512 C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_differential_avx2_vs_scalar_c_all(input in ".*") {
            if is_x86_feature_detected!("avx2") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::SingleByte);
                let simd = unsafe { crate::wc_x86::count_text_avx2(input.as_bytes(), LocaleEncoding::SingleByte) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX2 C: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX2 C: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX2 C: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX2 C: chars mismatch");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_differential_avx512_vs_scalar_c_all(input in ".*") {
            if is_x86_feature_detected!("avx512bw")&& is_x86_feature_detected!("avx512f") {
                let scalar = crate::wc_default::word_count_scalar(input.as_bytes(), LocaleEncoding::SingleByte);
                let simd = unsafe { crate::wc_x86::count_text_avx512(input.as_bytes(), LocaleEncoding::SingleByte) };

                prop_assert_eq!(scalar.lines, simd.lines, "AVX512 C: lines mismatch");
                prop_assert_eq!(scalar.words, simd.words, "AVX512 C: words mismatch");
                prop_assert_eq!(scalar.bytes, simd.bytes, "AVX512 C: bytes mismatch");
                prop_assert_eq!(scalar.chars, simd.chars, "AVX512 C: chars mismatch");
            }
        }
    }
}
