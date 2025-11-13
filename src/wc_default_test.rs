#[cfg(test)]
pub mod tests {
    use crate::wc_default::{detect_incomplete_utf8_suffix, word_count_scalar};
    use crate::{FileCounts, LocaleEncoding};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use rstest_reuse;
    use rstest_reuse::*;

    #[rstest]
    // Empty and ASCII cases
    #[case::empty_buffer(&[], 0)]
    #[case::ascii_only_complete(b"hello", 0)]
    #[case::ascii_with_space_and_numbers(b"test 123", 0)]
    // 2-byte UTF-8 sequences (110xxxxx 10xxxxxx)
    // é = C3 A9
    #[case::utf8_2byte_complete(&[0xC3, 0xA9], 0)]
    #[case::ascii_plus_utf8_2byte_complete(b"hello\xC3\xA9", 0)]
    #[case::utf8_2byte_incomplete_start_only(&[0xC3], 1)]
    #[case::ascii_plus_utf8_2byte_incomplete(b"hello\xC3", 1)]
    // 3-byte UTF-8 sequences (1110xxxx 10xxxxxx 10xxxxxx)
    // ✓ = E2 9C 93
    #[case::utf8_3byte_complete(&[0xE2, 0x9C, 0x93], 0)]
    #[case::ascii_plus_utf8_3byte_complete(b"test\xE2\x9C\x93", 0)]
    #[case::utf8_3byte_incomplete_1_byte(&[0xE2], 1)]
    #[case::utf8_3byte_incomplete_2_bytes(&[0xE2, 0x9C], 2)]
    #[case::ascii_plus_utf8_3byte_incomplete_1_byte(b"hello\xE2", 1)]
    #[case::ascii_plus_utf8_3byte_incomplete_2_bytes(b"hello\xE2\x9C", 2)]
    // 4-byte UTF-8 sequences (11110xxx 10xxxxxx 10xxxxxx 10xxxxxx)
    // 💯 = F0 9F 92 AF
    #[case::utf8_4byte_complete(&[0xF0, 0x9F, 0x92, 0xAF], 0)]
    #[case::ascii_plus_utf8_4byte_complete(b"test\xF0\x9F\x92\xAF", 0)]
    #[case::utf8_4byte_incomplete_1_byte(&[0xF0], 1)]
    #[case::utf8_4byte_incomplete_2_bytes(&[0xF0, 0x9F], 2)]
    #[case::utf8_4byte_incomplete_3_bytes(&[0xF0, 0x9F, 0x92], 3)]
    #[case::ascii_plus_utf8_4byte_incomplete_1_byte(b"hi\xF0", 1)]
    #[case::ascii_plus_utf8_4byte_incomplete_2_bytes(b"hi\xF0\x9F", 2)]
    #[case::ascii_plus_utf8_4byte_incomplete_3_bytes(b"hi\xF0\x9F\x92", 3)]
    // Continuation bytes only (10xxxxxx)
    #[case::single_continuation_byte(&[0x80], 1)]
    #[case::two_continuation_bytes(&[0x80, 0x80], 2)]
    #[case::three_continuation_bytes(&[0x80, 0x80, 0x80], 3)]
    #[case::four_continuation_bytes(&[0x80, 0x80, 0x80, 0x80], 4)]
    // Invalid UTF-8 start bytes
    #[case::invalid_utf8_byte_0xff(&[0xFF], 0)]
    #[case::invalid_utf8_byte_0xfe(&[0xFE], 0)]
    #[case::ascii_plus_invalid_utf8(b"test\xFF", 0)]
    // Multiple complete sequences
    #[case::two_complete_2byte_sequences(b"\xC3\xA9\xC3\xA9", 0)]
    #[case::two_complete_3byte_sequences(b"\xE2\x9C\x93\xE2\x9C\x93", 0)]
    // Edge case: ASCII followed by incomplete at exact boundary
    #[case::eleven_ascii_plus_1_incomplete(b"hello world\xE2", 1)]
    #[case::thirteen_ascii_plus_2_incomplete(b"hello world!!\xE2\x9C", 2)]
    // Real-world: checkmark split across chunk (from our earlier bug)
    #[case::chunk_boundary_14_bytes_plus_2_incomplete(b"hello world   \xE2\x9C", 2)]

    fn test_detect_incomplete_utf8_suffix(#[case] input: &[u8], #[case] expected: usize) {
        let result = detect_incomplete_utf8_suffix(input);
        assert_eq!(result, expected, "input: {:?}", input);
    }

    // Helper to create expected FileCounts
    pub fn counts(lines: usize, words: usize, chars: usize, bytes: usize) -> FileCounts {
        FileCounts {
            lines,
            words,
            chars,
            bytes,
        }
    }

    // Template: Common test cases for word counting
    // Can be reused in wc_default_test.rs and wc_arm64_test.rs
    #[template]
    #[rstest]
    // Empty and whitespace-only cases
    #[case::empty("", LocaleEncoding::Utf8, counts(0, 0, 0, 0))]
    #[case::empty_c_locale("", LocaleEncoding::C, counts(0, 0, 0, 0))]
    #[case::single_space(" ", LocaleEncoding::Utf8, counts(0, 0, 1, 1))]
    #[case::multiple_spaces("   ", LocaleEncoding::Utf8, counts(0, 0, 3, 3))]
    #[case::single_newline("\n", LocaleEncoding::Utf8, counts(1, 0, 1, 1))]
    #[case::multiple_newlines("\n\n\n", LocaleEncoding::Utf8, counts(3, 0, 3, 3))]
    #[case::spaces_and_newlines("  \n  \n", LocaleEncoding::Utf8, counts(2, 0, 6, 6))]
    // Single word cases
    #[case::single_word("hello", LocaleEncoding::Utf8, counts(0, 1, 5, 5))]
    #[case::single_word_with_newline("hello\n", LocaleEncoding::Utf8, counts(1, 1, 6, 6))]
    #[case::single_word_with_spaces(" hello ", LocaleEncoding::Utf8, counts(0, 1, 7, 7))]
    // Multiple words - ASCII only
    #[case::two_words("hello world", LocaleEncoding::Utf8, counts(0, 2, 11, 11))]
    #[case::three_words("one two three", LocaleEncoding::Utf8, counts(0, 3, 13, 13))]
    #[case::words_multiple_spaces("one  two   three", LocaleEncoding::Utf8, counts(0, 3, 16, 16))]
    #[case::words_with_newlines("one\ntwo\nthree", LocaleEncoding::Utf8, counts(2, 3, 13, 13))]
    // Tab and other ASCII whitespace (0x09-0x0D)
    #[case::words_with_tab("one\ttwo", LocaleEncoding::Utf8, counts(0, 2, 7, 7))]
    #[case::words_with_cr("one\rtwo", LocaleEncoding::Utf8, counts(0, 2, 7, 7))]
    #[case::words_with_vt("one\x0Btwo", LocaleEncoding::Utf8, counts(0, 2, 7, 7))]
    #[case::words_with_ff("one\x0Ctwo", LocaleEncoding::Utf8, counts(0, 2, 7, 7))]
    // UTF-8 locale: 2-byte characters
    // é = C3 A9 (2 bytes, 1 char)
    #[case::utf8_single_char_2byte("é", LocaleEncoding::Utf8, counts(0, 1, 1, 2))]
    #[case::utf8_word_with_2byte("café", LocaleEncoding::Utf8, counts(0, 1, 4, 5))]
    #[case::utf8_two_words_2byte("café résumé", LocaleEncoding::Utf8, counts(0, 2, 11, 14))]
    // UTF-8 locale: 3-byte characters
    // ✓ = E2 9C 93 (3 bytes, 1 char)
    #[case::utf8_single_char_3byte("✓", LocaleEncoding::Utf8, counts(0, 1, 1, 3))]
    #[case::utf8_word_with_3byte("test✓", LocaleEncoding::Utf8, counts(0, 1, 5, 7))]
    // UTF-8 locale: 4-byte characters (emojis)
    // 💯 = F0 9F 92 AF (4 bytes, 1 char)
    #[case::utf8_single_emoji("💯", LocaleEncoding::Utf8, counts(0, 1, 1, 4))]
    #[case::utf8_word_with_emoji("test💯", LocaleEncoding::Utf8, counts(0, 1, 5, 8))]
    #[case::utf8_emoji_sentence("hello 💯 world", LocaleEncoding::Utf8, counts(0, 3, 13, 16))]
    // UTF-8 locale: Unicode whitespace
    // U+00A0 (non-breaking space) = C2 A0
    #[case::utf8_non_breaking_space(
        "hello\u{00A0}world",
        LocaleEncoding::Utf8,
        counts(0, 2, 11, 12)
    )]
    // U+0085 (next line) = C2 85 - Unicode whitespace but not counted as \n
    #[case::utf8_next_line("hello\u{0085}world", LocaleEncoding::Utf8, counts(0, 2, 11, 12))]
    // U+2003 (em space) = E2 80 83
    #[case::utf8_em_space("hello\u{2003}world", LocaleEncoding::Utf8, counts(0, 2, 11, 13))]
    // UTF-8 replacement character (from our earlier bug discovery)
    // U+FFFD = EF BF BD (3 bytes, 1 char) - NOT whitespace, part of word
    #[case::utf8_replacement_char("test\u{FFFD}word", LocaleEncoding::Utf8, counts(0, 1, 9, 11))]
    #[case::utf8_two_replacement_chars(
        "word1\u{FFFD}word2\u{FFFD}word3",
        LocaleEncoding::Utf8,
        counts(0, 1, 17, 21)
    )]
    // C locale: bytes = chars
    #[case::c_locale_ascii("hello world", LocaleEncoding::C, counts(0, 2, 11, 11))]
    #[case::c_locale_utf8_bytes("café", LocaleEncoding::C, counts(0, 1, 5, 5))] // 5 bytes = 5 chars in C
    #[case::c_locale_emoji("💯", LocaleEncoding::C, counts(0, 1, 4, 4))] // 4 bytes = 4 chars in C

    // C locale: no Unicode whitespace detection (only ASCII)
    #[case::c_locale_non_breaking_space_no_split(
        "hello\u{00A0}world",
        LocaleEncoding::C,
        counts(0, 1, 12, 12)
    )] // Non-breaking space not recognized, 1 word

    // Mixed content
    #[case::mixed_ascii_utf8("hello café 💯 world", LocaleEncoding::Utf8, counts(0, 4, 18, 22))]
    #[case::mixed_with_newlines(
        "line1 café\nline2 💯\n",
        LocaleEncoding::Utf8,
        counts(2, 4, 19, 23)
    )]
    // Edge case: trailing whitespace
    #[case::trailing_space("hello ", LocaleEncoding::Utf8, counts(0, 1, 6, 6))]
    #[case::trailing_newline("hello\n", LocaleEncoding::Utf8, counts(1, 1, 6, 6))]
    #[case::trailing_multiple_spaces("hello   ", LocaleEncoding::Utf8, counts(0, 1, 8, 8))]
    // Edge case: leading whitespace
    #[case::leading_space(" hello", LocaleEncoding::Utf8, counts(0, 1, 6, 6))]
    #[case::leading_newline("\nhello", LocaleEncoding::Utf8, counts(1, 1, 6, 6))]
    pub fn common_word_count_cases(
        #[case] input: &str,
        #[case] locale: LocaleEncoding,
        #[case] expected: FileCounts,
    ) {
    }

    // Apply template to test scalar implementation
    #[apply(common_word_count_cases)]
    fn test_word_count_scalar(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        let result = word_count_scalar(input, locale);
        assert_eq!(result, expected);
    }
}
