#[cfg(test)]
mod tests {
    use crate::wc_default::detect_incomplete_utf8_suffix;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

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

    fn test_detect_incomplete_utf8_suffix(
        #[case] input: &[u8],
        #[case] expected: usize,
    ) {
        let result = detect_incomplete_utf8_suffix(input);
        assert_eq!(
            result, expected,
            "input: {:?}",
            input
        );
    }
}
