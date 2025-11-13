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
        let result = count_text_simd(input, locale).expect("simd should be available on x86_64");
        assert_eq!(result, expected);
    }
}
