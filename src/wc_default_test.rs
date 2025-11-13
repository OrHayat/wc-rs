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
    // Chunk boundary tests: UTF-8 characters split across 16-byte SIMD boundaries
    // These test incomplete UTF-8 handling at chunk boundaries
    // Test case: 15 ASCII bytes + 3-byte UTF-8 em-space (U+2003 = E2 80 83) + 5 ASCII
    // Total: 23 bytes, 21 chars, 2 words
    // The em-space will be split at 16-byte boundary: first chunk gets "E2", next gets "80 83"
    #[case::chunk_boundary_16_3byte_split(
        "123456789012345\u{2003}world",
        LocaleEncoding::Utf8,
        counts(0, 2, 21, 23)
    )]
    // Test case: 14 ASCII bytes + 2-byte UTF-8 nbsp (U+00A0 = C2 A0) + 5 ASCII
    // Total: 21 bytes, 20 chars, 2 words
    // The nbsp will be split at 16-byte boundary
    #[case::chunk_boundary_16_2byte_split(
        "12345678901234\u{00A0}world",
        LocaleEncoding::Utf8,
        counts(0, 2, 20, 21)
    )]
    // Test case: 13 ASCII bytes + 4-byte emoji (💯 = F0 9F 92 AF) + space + 5 ASCII
    // Total: 23 bytes, 20 chars, 2 words
    // The emoji will be split at 16-byte boundary: chunk1 gets "F0", chunk2 gets "9F 92 AF"
    #[case::chunk_boundary_16_4byte_split(
        "1234567890123💯 world",
        LocaleEncoding::Utf8,
        counts(0, 2, 20, 23)
    )]
    // ====== ADDITIONAL EDGE CASES ======
    // Mixed line endings
    // Note: Only \n counts as newline, \r is just a character (but still whitespace for word splitting)
    #[case::windows_line_ending("hello\r\nworld", LocaleEncoding::Utf8, counts(1, 2, 12, 12))]
    #[case::old_mac_line_ending("hello\rworld", LocaleEncoding::Utf8, counts(0, 2, 11, 11))]
    #[case::mixed_line_endings(
        "line1\nline2\r\nline3\r",
        LocaleEncoding::Utf8,
        counts(2, 3, 19, 19)
    )]
    #[case::multiple_crlf("a\r\nb\r\nc\r\n", LocaleEncoding::Utf8, counts(3, 3, 9, 9))]
    // Empty lines and consecutive newlines
    #[case::two_consecutive_newlines("\n\n", LocaleEncoding::Utf8, counts(2, 0, 2, 2))]
    #[case::word_empty_line_word("word1\n\nword2", LocaleEncoding::Utf8, counts(2, 2, 12, 12))]
    #[case::multiple_empty_lines("a\n\n\n\nb", LocaleEncoding::Utf8, counts(4, 2, 6, 6))]
    #[case::empty_line_with_spaces("word\n  \nword", LocaleEncoding::Utf8, counts(2, 2, 12, 12))]
    // Control characters (beyond basic whitespace)
    #[case::null_byte("hello\x00world", LocaleEncoding::Utf8, counts(0, 1, 11, 11))] // NULL is not whitespace
    #[case::bell_char("hello\x07world", LocaleEncoding::Utf8, counts(0, 1, 11, 11))] // BEL is not whitespace
    #[case::escape_char("hello\x1bworld", LocaleEncoding::Utf8, counts(0, 1, 11, 11))] // ESC is not whitespace
    // Very long words (stress SIMD boundaries)
    #[case::long_word_32_bytes(
        "abcdefghijklmnopqrstuvwxyz123456", // 32 bytes, 1 word
        LocaleEncoding::Utf8,
        counts(0, 1, 32, 32)
    )]
    #[case::long_word_64_bytes(
        "abcdefghijklmnopqrstuvwxyz1234567890abcdefghijklmnopqrstuvwxyz12", // 64 bytes
        LocaleEncoding::Utf8,
        counts(0, 1, 64, 64)
    )]
    #[case::long_word_with_space_after(
        "abcdefghijklmnopqrstuvwxyz123456 next",
        LocaleEncoding::Utf8,
        counts(0, 2, 37, 37)
    )]
    // Exact boundary alignments (16, 32, 48 bytes)
    #[case::exactly_16_bytes("1234567890123456", LocaleEncoding::Utf8, counts(0, 1, 16, 16))]
    #[case::exactly_32_bytes(
        "12345678901234567890123456789012",
        LocaleEncoding::Utf8,
        counts(0, 1, 32, 32)
    )]
    #[case::exactly_48_bytes(
        "123456789012345678901234567890123456789012345678",
        LocaleEncoding::Utf8,
        counts(0, 1, 48, 48)
    )]
    #[case::word_at_16_boundary("123456789012345 word", LocaleEncoding::Utf8, counts(0, 2, 20, 20))]
    #[case::word_at_32_boundary(
        "1234567890123456789012345678901 word",
        LocaleEncoding::Utf8,
        counts(0, 2, 36, 36)
    )]
    // Zero-width and special Unicode characters
    // IMPORTANT: Zero-width chars (U+200B, U+200C, U+200D, U+2060) are Unicode category Cf (Format)
    // They are NOT whitespace per char.is_whitespace(), so they DON'T split words!
    // U+200B = E2 80 8B (3 bytes), U+200D = E2 80 8D (3 bytes), etc.
    #[case::zero_width_space("hello\u{200B}world", LocaleEncoding::Utf8, counts(0, 1, 11, 13))] // U+200B (NOT whitespace!)
    #[case::zero_width_joiner("hello\u{200D}world", LocaleEncoding::Utf8, counts(0, 1, 11, 13))] // U+200D (not whitespace)
    #[case::zero_width_non_joiner("hello\u{200C}world", LocaleEncoding::Utf8, counts(0, 1, 11, 13))] // U+200C (not whitespace)
    #[case::byte_order_mark("\u{FEFF}hello", LocaleEncoding::Utf8, counts(0, 1, 6, 8))] // BOM U+FEFF = EF BB BF (3 bytes)
    #[case::word_joiner("hello\u{2060}world", LocaleEncoding::Utf8, counts(0, 1, 11, 13))] // U+2060 = E2 81 A0 (3 bytes, not whitespace)
    // Combining characters and diacritics
    #[case::combining_acute("e\u{0301}", LocaleEncoding::Utf8, counts(0, 1, 2, 3))] // e + combining acute = é
    #[case::word_with_combining("cafe\u{0301}", LocaleEncoding::Utf8, counts(0, 1, 5, 6))] // café
    #[case::multiple_combining("e\u{0301}\u{0302}", LocaleEncoding::Utf8, counts(0, 1, 3, 5))] // e with multiple accents
    // More Unicode whitespace variations
    #[case::thin_space("hello\u{2009}world", LocaleEncoding::Utf8, counts(0, 2, 11, 13))] // U+2009 thin space
    #[case::hair_space("hello\u{200A}world", LocaleEncoding::Utf8, counts(0, 2, 11, 13))] // U+200A hair space
    #[case::line_separator("hello\u{2028}world", LocaleEncoding::Utf8, counts(0, 2, 11, 13))] // U+2028 line separator
    #[case::paragraph_separator("hello\u{2029}world", LocaleEncoding::Utf8, counts(0, 2, 11, 13))] // U+2029 paragraph separator
    #[case::figure_space("hello\u{2007}world", LocaleEncoding::Utf8, counts(0, 2, 11, 13))] // U+2007 figure space
    #[case::ideographic_space("hello\u{3000}world", LocaleEncoding::Utf8, counts(0, 2, 11, 13))] // U+3000 ideographic space
    // Emoji variations and sequences
    #[case::emoji_skin_tone("👍🏽", LocaleEncoding::Utf8, counts(0, 1, 2, 8))]
    // Thumbs up + skin tone modifier
    // IMPORTANT: ZWJ emoji sequences count each scalar value separately!
    // Family emoji: 👨 + ZWJ + 👩 + ZWJ + 👧 + ZWJ + 👦 = 7 chars (not 1!)
    // Each emoji is 4 bytes, each ZWJ (U+200D) is 3 bytes: 4+3+4+3+4+3+4 = 25 bytes
    #[case::emoji_with_space("👨‍👩‍👧‍👦 family", LocaleEncoding::Utf8, counts(0, 2, 14, 32))] // 7 (emoji) + 1 (space) + 6 (family) = 14 chars
    #[case::multiple_emojis("😀😃😄", LocaleEncoding::Utf8, counts(0, 1, 3, 12))] // 3 emojis = 1 word
    #[case::emoji_separated("😀 😃 😄", LocaleEncoding::Utf8, counts(0, 3, 5, 14))] // 3 words
    // Maximum Unicode codepoint
    #[case::max_unicode("\u{10FFFF}", LocaleEncoding::Utf8, counts(0, 1, 1, 4))] // U+10FFFF (max valid)
    #[case::supplementary_plane("𐍈𐍈𐍈", LocaleEncoding::Utf8, counts(0, 1, 3, 12))] // Gothic letters (U+10348)
    // Alternating single and multi-byte patterns
    #[case::alternating_1_2_byte("aéaéaé", LocaleEncoding::Utf8, counts(0, 1, 6, 9))]
    #[case::alternating_1_3_byte("a✓a✓a✓", LocaleEncoding::Utf8, counts(0, 1, 6, 12))]
    #[case::alternating_1_4_byte("a💯a💯", LocaleEncoding::Utf8, counts(0, 1, 4, 10))] // a(1+1) + 💯(1+4) + a(1+1) + 💯(1+4) = 4 chars, 10 bytes
    #[case::alternating_with_spaces("a é ✓ 💯", LocaleEncoding::Utf8, counts(0, 4, 7, 13))]
    // Right-to-left text (Arabic, Hebrew)
    #[case::arabic_text("مرحبا العالم", LocaleEncoding::Utf8, counts(0, 2, 12, 23))] // "Hello World" in Arabic
    #[case::hebrew_text("שלום עולם", LocaleEncoding::Utf8, counts(0, 2, 9, 17))] // "Hello World" in Hebrew: 4 + space + 4 = 9 chars
    #[case::mixed_ltr_rtl("hello مرحبا world", LocaleEncoding::Utf8, counts(0, 3, 17, 22))]
    // Pathological whitespace sequences
    #[case::many_space_types(
        "word1 \t\n\r\u{00A0}\u{2003}\u{3000}word2",
        LocaleEncoding::Utf8,
        counts(1, 2, 17, 22)
    )]
    #[case::whitespace_soup(
        "\t \n \r \u{00A0} \u{2009} \u{200A}",
        LocaleEncoding::Utf8,
        counts(1, 0, 11, 16)
    )]
    // Words with punctuation (punctuation is not whitespace)
    #[case::word_with_comma("hello,world", LocaleEncoding::Utf8, counts(0, 1, 11, 11))]
    #[case::word_with_period("hello.world", LocaleEncoding::Utf8, counts(0, 1, 11, 11))]
    #[case::sentence("Hello, world!", LocaleEncoding::Utf8, counts(0, 2, 13, 13))]
    #[case::quoted_word("\"hello\"", LocaleEncoding::Utf8, counts(0, 1, 7, 7))]
    // Note: Invalid UTF-8 sequences tested separately in detect_incomplete_utf8_suffix tests
    // String literals must be valid UTF-8, but invalid sequences are handled at byte level
    // C locale specific edge cases
    #[case::c_locale_with_newline("hello\nworld", LocaleEncoding::C, counts(1, 2, 11, 11))] // Test newline counting in C locale
    #[case::c_locale_multiple_newlines("a\nb\nc\n", LocaleEncoding::C, counts(3, 3, 6, 6))] // Multiple newlines in C locale
    #[case::c_locale_multibyte_as_bytes(
        "café\u{2003}test",
        LocaleEncoding::C,
        counts(0, 1, 12, 12)
    )] // café(5) + em-space(3) + test(4) = 12 bytes = 12 chars in C
    #[case::c_locale_emoji_as_bytes("💯test", LocaleEncoding::C, counts(0, 1, 8, 8))] // 4 emoji bytes + 4 ASCII
    #[case::c_locale_only_ascii_whitespace(
        "word1\u{00A0}word2",
        LocaleEncoding::C,
        counts(0, 1, 12, 12)
    )] // nbsp not recognized
    // C locale with ASCII >= 16 bytes (tests SIMD path on ARM64 NEON)
    #[case::c_locale_ascii_16_bytes("abcdefghijklmnop", LocaleEncoding::C, counts(0, 1, 16, 16))] // Exactly 16 bytes
    #[case::c_locale_ascii_32_bytes(
        "hello world test data here!!",
        LocaleEncoding::C,
        counts(0, 5, 28, 28)
    )] // 28 bytes, multiple chunks
    #[case::c_locale_ascii_48_bytes(
        "the quick brown fox jumps over the lazy dog here",
        LocaleEncoding::C,
        counts(0, 10, 48, 48)
    )] // 48 bytes
    // Extreme cases
    #[case::only_replacement_chars(
        "\u{FFFD}\u{FFFD}\u{FFFD}",
        LocaleEncoding::Utf8,
        counts(0, 1, 3, 9)
    )]
    // Zero-width spaces are NOT whitespace, so they form a word (not empty!)
    #[case::only_zero_width_spaces(
        "\u{200B}\u{200B}\u{200B}",
        LocaleEncoding::Utf8,
        counts(0, 1, 3, 9)
    )]
    #[case::word_ending_at_exact_16("12345678901234 w", LocaleEncoding::Utf8, counts(0, 2, 16, 16))]
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
