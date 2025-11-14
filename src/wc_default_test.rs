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
    // Single character cases
    #[case::single_ascii_char("a", LocaleEncoding::Utf8, counts(0, 1, 1, 1))]
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
    // PropTest regression: AVX2 has_non_ascii bug (missing negative byte check for 0x80-0xFF)
    // 4-byte UTF-8 char split across 32-byte AVX2 boundary (byte 29-32: f0 90 a3 a0)
    #[case::avx2_regression_complex_unicode(
        "A ࠰®0᥀ 𑊟᰻0🪀¡0®𐣠A",
        LocaleEncoding::Utf8,
        counts(0, 3, 16, 34)
    )]
    // PropTest regression: AVX512 has_non_ascii bug (same issue as AVX2)
    // Complex Unicode with various multi-byte characters
    #[case::avx512_regression_complex_unicode(
        "a \u{c55}𐀼\u{cbc}a𐖌aa🉠𞹟🠀a𝼀® ฿ጒ a🌀 ®   𞅎\u{1e01b}𐨕",
        LocaleEncoding::Utf8,
        counts(0, 6, 29, 69)
    )]
    pub fn common_word_count_cases(
        #[case] input: &str,
        #[case] locale: LocaleEncoding,
        #[case] expected: FileCounts,
    ) {
    }

    // Apply template to test scalar implementation
    #[apply(common_word_count_cases)]
    fn test_word_count_scalar(input: &str, locale: LocaleEncoding, expected: FileCounts) {
        let result = word_count_scalar(input.as_bytes(), locale);
        assert_eq!(result, expected);
    }

    // ====================================================================
    // Invalid UTF-8 Test Cases (using raw bytes)
    // ====================================================================

    #[rstest]
    // Lone continuation byte (10xxxxxx without start byte)
    // GNU wc: isolated invalid bytes don't form words
    #[case::invalid_lone_continuation(
        &[0x80u8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 1)  // lines=0, words=0, chars=0, bytes=1
    )]
    // Truncated 2-byte sequence (110xxxxx without continuation)
    // Treated as incomplete at end, not processed
    #[case::invalid_truncated_2byte(
        &[0xC2u8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 1)  // lines=0, words=0, chars=0, bytes=1
    )]
    // Truncated 3-byte sequence (1110xxxx + one continuation)
    #[case::invalid_truncated_3byte(
        &[0xE0u8, 0xA0u8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 2)  // lines=0, words=0, chars=0, bytes=2
    )]
    // Truncated 4-byte sequence (11110xxx + two continuations)
    #[case::invalid_truncated_4byte(
        &[0xF0u8, 0x90u8, 0x80u8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 3)  // lines=0, words=0, chars=0, bytes=3
    )]
    // Invalid byte in middle of ASCII word
    #[case::invalid_in_word(
        b"hello\xFFworld",
        LocaleEncoding::Utf8,
        counts(0, 1, 10, 11)  // 1 word (joined), 10 chars (h,e,l,l,o,w,o,r,l,d), 11 bytes
    )]
    // Invalid bytes separated by space: isolated invalid bytes don't form words
    #[case::invalid_separated_by_space(
        &[0xFFu8, b' ', 0xFEu8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 1, 3)  // 0 words, 1 char (space), 3 bytes
    )]
    // Invalid byte at start
    #[case::invalid_at_start(
        b"\xFFhello",
        LocaleEncoding::Utf8,
        counts(0, 1, 5, 6)  // 1 word, 5 chars (hello), 6 bytes
    )]
    // Invalid byte at end
    #[case::invalid_at_end(
        b"hello\xFF",
        LocaleEncoding::Utf8,
        counts(0, 1, 5, 6)  // 1 word, 5 chars (hello), 6 bytes
    )]
    // Mixed valid UTF-8 and invalid bytes
    #[case::mixed_valid_invalid(
        b"caf\xC3\xA9\xFF\xFEtest",  // café with invalid bytes after
        LocaleEncoding::Utf8,
        counts(0, 1, 8, 11)  // 1 word, 8 chars (c,a,f,é,t,e,s,t), 11 bytes
    )]
    // C locale: invalid bytes ARE chars
    #[case::invalid_c_locale(
        &[0xFFu8, 0xFEu8, 0xFDu8][..],
        LocaleEncoding::C,
        counts(0, 1, 3, 3)  // C locale: all bytes are chars
    )]
    // Invalid byte then newline: isolated invalid byte doesn't form word
    #[case::invalid_then_newline(
        b"\xFF\n",
        LocaleEncoding::Utf8,
        counts(1, 0, 1, 2)  // 1 line, 0 words, 1 char (newline), 2 bytes
    )]
    // Multiple invalid bytes in sequence
    #[case::multiple_invalid_consecutive(
        &[0xFFu8, 0xFEu8, 0xFDu8, 0xFCu8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 4)  // 0 words (isolated invalid bytes), 0 chars, 4 bytes
    )]
    // Invalid bytes joining two words
    #[case::invalid_joining_two_words(
        b"hello\xFF\xFE\xFDworld",
        LocaleEncoding::Utf8,
        counts(0, 1, 10, 13)  // 1 word (joined by invalid bytes), 10 chars, 13 bytes
    )]
    // Invalid byte after whitespace before word
    #[case::space_invalid_word(
        b" \xFFhello",
        LocaleEncoding::Utf8,
        counts(0, 1, 6, 7)  // 1 word (space + invalid + hello), 6 chars, 7 bytes
    )]
    // Word, invalid, space, word
    #[case::word_invalid_space_word(
        b"hello\xFF world",
        LocaleEncoding::Utf8,
        counts(0, 2, 11, 12)  // 2 words, 11 chars, 12 bytes
    )]
    // Invalid start byte 0xF8-0xFF (invalid in UTF-8)
    #[case::invalid_start_byte_f8(
        &[0xF8u8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 1)
    )]
    // Wrong continuation byte pattern
    #[case::wrong_continuation(
        &[0xC2u8, 0xC2u8][..],  // Start byte followed by another start byte
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 2)
    )]
    // Valid 2-byte followed by invalid
    #[case::valid_2byte_then_invalid(
        b"\xC3\xA9\xFF",  // é followed by invalid
        LocaleEncoding::Utf8,
        counts(0, 1, 1, 3)  // 1 word (joined), 1 char (é), 3 bytes
    )]
    // Invalid in middle of multibyte char
    #[case::invalid_breaks_multibyte(
        b"test\xC3\xFF\xA9more",  // Broken é sequence
        LocaleEncoding::Utf8,
        counts(0, 1, 8, 11)  // 1 word (all joined), 8 chars (test+more), 11 bytes
    )]
    // SIMD boundary: 15 valid + invalid at position 16
    #[case::invalid_at_16byte_boundary(
        b"123456789012345\xFF",  // 15 ASCII + invalid at position 16
        LocaleEncoding::Utf8,
        counts(0, 1, 15, 16)  // 1 word, 15 chars, 16 bytes
    )]
    // SIMD boundary: 31 valid + invalid at position 32
    #[case::invalid_at_32byte_boundary(
        b"1234567890123456789012345678901\xFF",  // 31 ASCII + invalid
        LocaleEncoding::Utf8,
        counts(0, 1, 31, 32)  // 1 word, 31 chars, 32 bytes
    )]
    // SIMD boundary: 63 valid + invalid at position 64
    #[case::invalid_at_64byte_boundary(
        b"123456789012345678901234567890123456789012345678901234567890123\xFF",  // 63 ASCII + invalid
        LocaleEncoding::Utf8,
        counts(0, 1, 63, 64)  // 1 word, 63 chars, 64 bytes
    )]
    // Invalid across newlines
    #[case::invalid_multiline(
        b"line1\xFF\nline2\xFE\n",
        LocaleEncoding::Utf8,
        counts(2, 2, 12, 14)  // 2 lines, 2 words, 12 chars, 14 bytes
    )]
    // Mix of tabs, spaces, invalid
    #[case::invalid_with_tabs(
        b"word1\t\xFF\tword2",
        LocaleEncoding::Utf8,
        counts(0, 2, 12, 13)  // 2 words, 12 chars (tabs count), 13 bytes
    )]
    // Overlong encoding (2-byte encoding of ASCII 'A' = 0xC1 0x81)
    #[case::overlong_2byte_encoding(
        &[0xC1u8, 0x81u8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 2)  // Invalid encoding, 0 chars
    )]
    // Surrogate half (0xED 0xA0 0x80) - invalid in UTF-8
    #[case::surrogate_half(
        &[0xEDu8, 0xA0u8, 0x80u8][..],
        LocaleEncoding::Utf8,
        counts(0, 0, 0, 3)  // Invalid surrogate
    )]
    // Valid emoji followed by invalid
    #[case::emoji_then_invalid(
        b"\xF0\x9F\x98\x80\xFF",  // 😀 followed by invalid
        LocaleEncoding::Utf8,
        counts(0, 1, 1, 5)  // 1 word (joined), 1 char (emoji), 5 bytes
    )]
    // Multiple words with invalid bytes between
    #[case::multiple_words_invalid_between(
        b"one\xFF two\xFE three",
        LocaleEncoding::Utf8,
        counts(0, 3, 13, 15)  // 3 words, 13 chars, 15 bytes
    )]
    fn test_word_count_scalar_invalid_utf8(
        #[case] input: &[u8],
        #[case] locale: LocaleEncoding,
        #[case] expected: FileCounts,
    ) {
        let result = word_count_scalar(input, locale);
        assert_eq!(result, expected);
    }

    // ====================================================================
    // Property-Based Tests (PropTest)
    // ====================================================================
    use proptest::prelude::*;

    // Property 1: bytes should equal input length
    proptest! {
        #[test]
        fn prop_bytes_equals_input_length_scalar(input in "\\PC*") {
            let result = word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            prop_assert_eq!(result.bytes, input.len(),
                "bytes should match input length");
        }
    }

    // Property 2: bytes >= chars >= lines (UTF-8 uses 1-4 bytes per char, newlines are chars)
    proptest! {
        #[test]
        fn prop_bytes_ge_chars_ge_lines_scalar(input in "\\PC*") {
            let result = word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            prop_assert!(result.bytes >= result.chars,
                "bytes ({}) must be >= chars ({})", result.bytes, result.chars);
            prop_assert!(result.chars >= result.lines,
                "chars ({}) must be >= lines ({})", result.chars, result.lines);
        }
    }

    // Property 3: C locale - lines <= chars == bytes (every byte is a char)
    proptest! {
        #[test]
        fn prop_c_locale_lines_le_chars_eq_bytes_scalar(input in "\\PC*") {
            let result = word_count_scalar(input.as_bytes(), LocaleEncoding::C);
            prop_assert_eq!(result.chars, result.bytes,
                "C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
            prop_assert!(result.lines <= result.chars,
                "C locale: lines ({}) must be <= chars ({})", result.lines, result.chars);
        }
    }

    // Property 4a: Line counting accuracy - no newlines (printable chars only)
    proptest! {
        #[test]
        fn prop_lines_zero_no_newlines_scalar(input in "\\PC*") {
            let result = word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            prop_assert_eq!(result.lines, 0,
                "no newlines: lines must be 0, got {}", result.lines);
        }
    }

    // Property 4b: Line counting accuracy - with newlines
    proptest! {
        #[test]
        fn prop_lines_count_accurate_scalar(input in ".*") {
            let result = word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            let expected_lines = input.chars().filter(|&c| c == '\n').count();
            prop_assert_eq!(result.lines, expected_lines,
                "lines ({}) must equal newline count ({})", result.lines, expected_lines);
        }
    }

    // Property 5: ASCII fast path - all bytes < 0x80 means bytes == chars
    proptest! {
        #[test]
        fn prop_ascii_bytes_eq_chars_scalar(input in "[\\x00-\\x7F]*") {
            let result = word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            prop_assert_eq!(result.bytes, result.chars,
                "ASCII: bytes ({}) must equal chars ({})", result.bytes, result.chars);
        }
    }

    // Property 6: All whitespace → words == 0 (and verify other counts)
    proptest! {
        #[test]
        fn prop_whitespace_zero_words_scalar(input in "\\s*") {
            let result = word_count_scalar(input.as_bytes(), LocaleEncoding::Utf8);
            prop_assert_eq!(result.words, 0,
                "all whitespace: words must be 0, got {}", result.words);
            prop_assert_eq!(result.bytes, input.len(),
                "all whitespace: bytes ({}) must equal input length ({})", result.bytes, input.len());
            prop_assert_eq!(result.chars, input.chars().count(),
                "all whitespace: chars ({}) must equal char count ({})", result.chars, input.chars().count());
            let expected_lines = input.chars().filter(|&c| c == '\n').count();
            prop_assert_eq!(result.lines, expected_lines,
                "all whitespace: lines ({}) must equal newline count ({})", result.lines, expected_lines);
        }
    }

    // ====================================================================
    // Property-Based Tests for Invalid UTF-8
    // ====================================================================
    use proptest::collection::vec as prop_vec;

    // Property 7: Invalid UTF-8 → bytes >= chars (invalid bytes don't count as chars)
    proptest! {
        #[test]
        fn prop_invalid_utf8_bytes_ge_chars_scalar(invalid_bytes in prop_vec(0u8..=255u8, 0..100)) {
            let result = word_count_scalar(&invalid_bytes, LocaleEncoding::Utf8);
            prop_assert!(result.bytes >= result.chars,
                "Invalid UTF-8: bytes ({}) must be >= chars ({})", result.bytes, result.chars);
        }
    }

    // Property 8: C locale with any bytes → chars == bytes (every byte is a char)
    proptest! {
        #[test]
        fn prop_c_locale_any_bytes_chars_eq_bytes_scalar(bytes in prop_vec(0u8..=255u8, 0..100)) {
            let result = word_count_scalar(&bytes, LocaleEncoding::C);
            prop_assert_eq!(result.chars, result.bytes,
                "C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);
        }
    }

    // Property 9: Invalid UTF-8 bytes are non-whitespace (join words)
    proptest! {
        #[test]
        fn prop_invalid_utf8_joins_words_scalar(
            prefix in "[a-z]+",
            invalid_byte in 0x80u8..=0xFFu8,  // Invalid UTF-8 start bytes
            suffix in "[a-z]+"
        ) {
            // Create: "prefix<invalid_byte>suffix" - invalid byte should NOT split words
            let mut bytes = Vec::new();
            bytes.extend_from_slice(prefix.as_bytes());
            bytes.push(invalid_byte);
            bytes.extend_from_slice(suffix.as_bytes());

            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // Invalid byte joins words (treated as non-whitespace)
            prop_assert_eq!(result.words, 1,
                "Invalid UTF-8 byte 0x{:02X} should join words, got {} words", invalid_byte, result.words);

            // Verify chars count: prefix.len() + suffix.len() (invalid byte NOT counted)
            let expected_chars = prefix.chars().count() + suffix.chars().count();
            prop_assert_eq!(result.chars, expected_chars,
                "Invalid UTF-8: chars should be {} (prefix + suffix), got {}", expected_chars, result.chars);

            // Verify bytes count: includes the invalid byte
            prop_assert_eq!(result.bytes, bytes.len(),
                "Invalid UTF-8: bytes should be {}, got {}", bytes.len(), result.bytes);
        }
    }

    // Property 10: Lone continuation bytes (0x80-0xBF) don't form words when isolated
    proptest! {
        #[test]
        fn prop_lone_continuation_bytes_scalar(
            continuation_byte in 0x80u8..=0xBFu8,
            count in 1usize..10
        ) {
            let bytes = vec![continuation_byte; count];
            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // Isolated continuation bytes: 0 words, 0 chars, N bytes
            prop_assert_eq!(result.words, 0,
                "Lone continuation bytes should not form words, got {}", result.words);
            prop_assert_eq!(result.chars, 0,
                "Lone continuation bytes should not count as chars, got {}", result.chars);
            prop_assert_eq!(result.bytes, count,
                "Bytes count should be {}, got {}", count, result.bytes);
        }
    }

    // Property 11: Truncated UTF-8 sequences at end
    proptest! {
        #[test]
        fn prop_truncated_sequences_at_end_scalar(
            prefix in "[a-z]{0,20}",
            start_byte in prop::sample::select(vec![
                0xC2u8, 0xC3u8,  // 2-byte starts
                0xE0u8, 0xE1u8,  // 3-byte starts
                0xF0u8, 0xF1u8,  // 4-byte starts
            ])
        ) {
            // Create input with truncated sequence at end
            let mut bytes = Vec::from(prefix.as_bytes());
            bytes.push(start_byte);

            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // Truncated at end: not processed, but bytes still counted
            prop_assert_eq!(result.bytes, bytes.len(),
                "Bytes should be {}, got {}", bytes.len(), result.bytes);

            // Chars should only count the valid prefix
            let expected_chars = prefix.chars().count();
            prop_assert_eq!(result.chars, expected_chars,
                "Chars should be {} (prefix only), got {}", expected_chars, result.chars);
        }
    }

    // Property 12: Invalid bytes don't increment line count (only \n does)
    proptest! {
        #[test]
        fn prop_invalid_bytes_no_lines_scalar(
            invalid_bytes in prop_vec(0x80u8..=0xFFu8, 1..50)
        ) {
            let result = word_count_scalar(&invalid_bytes, LocaleEncoding::Utf8);

            // No newlines in invalid bytes → 0 lines
            prop_assert_eq!(result.lines, 0,
                "Invalid bytes without \\n should have 0 lines, got {}", result.lines);
        }
    }

    // Property 13: Mix of valid ASCII and invalid bytes
    proptest! {
        #[test]
        fn prop_mixed_ascii_invalid_scalar(
            valid in "[a-z]{1,20}",
            invalid_count in 1usize..5
        ) {
            let mut bytes = Vec::from(valid.as_bytes());
            // Append invalid bytes
            for _ in 0..invalid_count {
                bytes.push(0xFF);
            }

            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // Should form 1 word (invalid bytes join with valid)
            prop_assert_eq!(result.words, 1,
                "Valid + invalid should form 1 word, got {}", result.words);

            // Chars = only valid ASCII chars
            prop_assert_eq!(result.chars, valid.len(),
                "Chars should be {} (valid only), got {}", valid.len(), result.chars);

            // Bytes = all bytes
            prop_assert_eq!(result.bytes, valid.len() + invalid_count,
                "Bytes should be {}, got {}", valid.len() + invalid_count, result.bytes);
        }
    }

    // Property 14: Invalid bytes preserve newline counting
    proptest! {
        #[test]
        fn prop_invalid_with_newlines_scalar(
            lines in prop_vec("[a-z]{0,10}", 1..10)
        ) {
            // Build input: line1\xFF\nline2\xFF\n...
            let mut bytes = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                bytes.extend_from_slice(line.as_bytes());
                bytes.push(0xFF);  // Invalid byte
                if i < lines.len() - 1 {
                    bytes.push(b'\n');
                }
            }

            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // Line count = number of \n bytes
            let expected_lines = lines.len() - 1;
            prop_assert_eq!(result.lines, expected_lines,
                "Lines should be {}, got {}", expected_lines, result.lines);

            // Chars = only valid chars (ASCII letters + newlines)
            let total_ascii: usize = lines.iter().map(|s| s.len()).sum();
            let expected_chars = total_ascii + expected_lines;  // letters + newlines
            prop_assert_eq!(result.chars, expected_chars,
                "Chars should be {}, got {}", expected_chars, result.chars);
        }
    }

    // Property 15: Invalid start bytes 0xF5-0xFF (always invalid in UTF-8)
    proptest! {
        #[test]
        fn prop_high_invalid_start_bytes_scalar(
            invalid_byte in 0xF5u8..=0xFFu8,
            count in 1usize..10
        ) {
            let bytes = vec![invalid_byte; count];
            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // These bytes are never valid UTF-8 start bytes
            prop_assert_eq!(result.chars, 0,
                "High invalid bytes should not count as chars, got {}", result.chars);
            prop_assert_eq!(result.words, 0,
                "Isolated invalid bytes should not form words, got {}", result.words);
            prop_assert_eq!(result.bytes, count,
                "Bytes should be {}, got {}", count, result.bytes);
        }
    }

    // Property 16: Overlong encodings are invalid
    proptest! {
        #[test]
        fn prop_overlong_encodings_invalid_scalar(
            ascii_char in 0x00u8..=0x7Fu8
        ) {
            // Create 2-byte overlong encoding of ASCII char
            // Valid ASCII: 0xxxxxxx
            // Overlong: 110000xx 10xxxxxx
            let overlong = vec![
                0xC0 | (ascii_char >> 6),
                0x80 | (ascii_char & 0x3F),
            ];

            let result = word_count_scalar(&overlong, LocaleEncoding::Utf8);

            // Overlong encoding is invalid
            prop_assert_eq!(result.chars, 0,
                "Overlong encoding should not count as char, got {}", result.chars);
            prop_assert_eq!(result.bytes, 2,
                "Bytes should be 2, got {}", result.bytes);
        }
    }

    // Property 17: Random byte sequences - bytes always >= chars
    proptest! {
        #[test]
        fn prop_random_bytes_invariant_scalar(
            bytes in prop_vec(0u8..=255u8, 1..200)
        ) {
            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // Fundamental invariant: bytes >= chars for any input
            prop_assert!(result.bytes >= result.chars,
                "bytes ({}) must be >= chars ({})", result.bytes, result.chars);

            // bytes count must match input length
            prop_assert_eq!(result.bytes, bytes.len(),
                "bytes must equal input length: {} != {}", result.bytes, bytes.len());

            // lines <= chars (newlines are chars in UTF-8)
            prop_assert!(result.lines <= result.chars,
                "lines ({}) must be <= chars ({})", result.lines, result.chars);
        }
    }

    // Property 18: Invalid bytes at SIMD boundaries (16, 32, 64)
    proptest! {
        #[test]
        fn prop_invalid_at_simd_boundaries_scalar(
            boundary_size in prop::sample::select(vec![16usize, 32, 64]),
            invalid_byte in 0x80u8..=0xFFu8
        ) {
            // Create: N-1 ASCII chars + 1 invalid byte at boundary
            let mut bytes = vec![b'a'; boundary_size - 1];
            bytes.push(invalid_byte);

            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // Should form 1 word
            prop_assert_eq!(result.words, 1,
                "Should form 1 word at boundary {}, got {}", boundary_size, result.words);

            // Chars = boundary_size - 1 (all except invalid)
            prop_assert_eq!(result.chars, boundary_size - 1,
                "Chars should be {}, got {}", boundary_size - 1, result.chars);

            // Bytes = boundary_size
            prop_assert_eq!(result.bytes, boundary_size,
                "Bytes should be {}, got {}", boundary_size, result.bytes);
        }
    }

    // Property 19: Multiple invalid bytes between words
    proptest! {
        #[test]
        fn prop_multiple_invalid_between_words_scalar(
            word1 in "[a-z]{1,10}",
            word2 in "[a-z]{1,10}",
            invalid_count in 1usize..10
        ) {
            let mut bytes = Vec::from(word1.as_bytes());
            // Add multiple invalid bytes
            for _ in 0..invalid_count {
                bytes.push(0xFF);
            }
            bytes.extend_from_slice(word2.as_bytes());

            let result = word_count_scalar(&bytes, LocaleEncoding::Utf8);

            // Invalid bytes join the words → 1 word
            prop_assert_eq!(result.words, 1,
                "Multiple invalid bytes should join words, got {}", result.words);

            // Chars = word1 + word2 (no invalid bytes)
            let expected_chars = word1.len() + word2.len();
            prop_assert_eq!(result.chars, expected_chars,
                "Chars should be {}, got {}", expected_chars, result.chars);
        }
    }

    // Property 20: C locale treats all bytes as chars, even invalid UTF-8
    proptest! {
        #[test]
        fn prop_c_locale_comprehensive_scalar(
            bytes in prop_vec(0u8..=255u8, 1..200)
        ) {
            let result = word_count_scalar(&bytes, LocaleEncoding::C);

            // C locale: every byte is a char
            prop_assert_eq!(result.chars, result.bytes,
                "C locale: chars ({}) must equal bytes ({})", result.chars, result.bytes);

            // Line count = count of \n bytes
            let expected_lines = bytes.iter().filter(|&&b| b == b'\n').count();
            prop_assert_eq!(result.lines, expected_lines,
                "C locale: lines ({}) must equal \\n count ({})", result.lines, expected_lines);
        }
    }
}
