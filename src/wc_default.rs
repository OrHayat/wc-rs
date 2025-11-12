use crate::{FileCounts, LocaleEncoding};

/// Result from scalar word counting with incomplete UTF-8 sequence tracking
#[derive(Debug)]
pub(crate) struct ScalarResult {
    pub counts: FileCounts,
    pub incomplete_bytes: usize,  // 0-3 bytes at end that are incomplete UTF-8
    pub seen_space: bool,         // Last complete character was whitespace
}

pub fn word_count_scalar(content: &str, locale: LocaleEncoding) -> FileCounts {
    word_count_scalar_with_state(content.as_bytes(), true, locale).counts
}

pub(crate) fn word_count_scalar_with_state(content: &[u8], initial_seen_space: bool, locale: LocaleEncoding) -> ScalarResult {
    match locale {
        LocaleEncoding::C => word_count_scalar_bytes(content, initial_seen_space),
        LocaleEncoding::Utf8 => word_count_scalar_chars(content, initial_seen_space),
    }
}

/// Count words/lines/chars in C locale (byte mode - every byte is a character)
fn word_count_scalar_bytes(content: &[u8], initial_seen_space: bool) -> ScalarResult {
    let mut counts = FileCounts {
        lines: 0,
        words: 0,
        bytes: content.len(),
        chars: content.len(),  // C locale: bytes = chars
    };
    let mut seen_space = initial_seen_space;

    // Iterate over bytes
    for &byte in content {
        if byte == b'\n' {
            counts.lines += 1;
        }

        // Check if byte is ASCII whitespace
        let is_ws = matches!(byte, b' ' | 0x09..=0x0d);

        if is_ws {
            seen_space = true;
        } else if seen_space {
            counts.words += 1;
            seen_space = false;
        }
    }

    ScalarResult {
        counts,
        incomplete_bytes: 0,  // C locale: no incomplete UTF-8
        seen_space,
    }
}

/// Count words/lines/chars in UTF-8 mode (iterate over characters, not bytes)
fn word_count_scalar_chars(content: &[u8], initial_seen_space: bool) -> ScalarResult {
    // Detect incomplete UTF-8 at end
    let incomplete = detect_incomplete_utf8_suffix(content);
    let valid_len = content.len() - incomplete;

    // Decode only the complete portion
    let valid_str = std::str::from_utf8(&content[0..valid_len])
        .expect("UTF-8 validation should succeed for valid portion");

    let mut counts = FileCounts {
        lines: 0,
        words: 0,
        bytes: content.len(),
        chars: 0,
    };
    let mut seen_space = initial_seen_space;

    // Iterate over Unicode characters
    for ch in valid_str.chars() {
        counts.chars += 1;

        if ch == '\n' {
            counts.lines += 1;
        }

        if ch.is_whitespace() {
            seen_space = true;
        } else if seen_space {
            counts.words += 1;
            seen_space = false;
        }
    }

    ScalarResult {
        counts,
        incomplete_bytes: incomplete,
        seen_space,
    }
}

/// Detect incomplete UTF-8 sequence at the end of a byte buffer.
/// Returns the number of bytes (0-4) at the end that form an incomplete sequence.
///
/// UTF-8 encoding:
/// - 1-byte: 0xxxxxxx (ASCII)
/// - 2-byte: 110xxxxx 10xxxxxx
/// - 3-byte: 1110xxxx 10xxxxxx 10xxxxxx
/// - 4-byte: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
pub(crate) fn detect_incomplete_utf8_suffix(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }

    // UTF-8 sequences can be at most 4 bytes, so check the last 4 bytes
    let start_pos = data.len().saturating_sub(4);

    // Scan backwards from the end looking for a UTF-8 start byte
    for pos in (start_pos..data.len()).rev() {
        let byte = data[pos];
        let bytes_from_here = data.len() - pos;

        match classify_utf8_byte(byte) {
            Utf8ByteType::Ascii => {
                // Single-byte character - everything is complete
                return 0;
            }
            Utf8ByteType::Start2Byte => {
                // Need 2 bytes total
                return if bytes_from_here < 2 { bytes_from_here } else { 0 };
            }
            Utf8ByteType::Start3Byte => {
                // Need 3 bytes total
                return if bytes_from_here < 3 { bytes_from_here } else { 0 };
            }
            Utf8ByteType::Start4Byte => {
                // Need 4 bytes total
                return if bytes_from_here < 4 { bytes_from_here } else { 0 };
            }
            Utf8ByteType::Continuation => {
                // Keep looking for the start byte
                continue;
            }
            Utf8ByteType::Invalid => {
                // Invalid UTF-8 - treat as complete
                return 0;
            }
        }
    }

    // All scanned bytes are continuation bytes - incomplete sequence
    data.len() - start_pos
}

/// Classification of a UTF-8 byte
#[derive(Debug, PartialEq)]
enum Utf8ByteType {
    Ascii,        // 0xxxxxxx
    Start2Byte,   // 110xxxxx
    Start3Byte,   // 1110xxxx
    Start4Byte,   // 11110xxx
    Continuation, // 10xxxxxx
    Invalid,      // Invalid start byte
}

/// Classify a byte according to UTF-8 encoding rules
#[inline]
fn classify_utf8_byte(byte: u8) -> Utf8ByteType {
    if byte & 0b10000000 == 0 {
        Utf8ByteType::Ascii
    } else if byte & 0b11100000 == 0b11000000 {
        Utf8ByteType::Start2Byte
    } else if byte & 0b11110000 == 0b11100000 {
        Utf8ByteType::Start3Byte
    } else if byte & 0b11111000 == 0b11110000 {
        Utf8ByteType::Start4Byte
    } else if byte & 0b11000000 == 0b10000000 {
        Utf8ByteType::Continuation
    } else {
        Utf8ByteType::Invalid
    }
}