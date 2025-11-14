use std::intrinsics::simd::simd_relaxed_fma;

use crate::{FileCounts, LocaleEncoding};

/// Result from scalar word counting with incomplete UTF-8 sequence tracking
#[derive(Debug)]
pub(crate) struct ScalarResult {
    pub counts: FileCounts,
    pub incomplete_bytes: usize, // 0-3 bytes at end that are incomplete UTF-8
    pub seen_space: bool,        // Last complete character was whitespace
}

pub fn word_count_scalar(content: &[u8], locale: LocaleEncoding) -> FileCounts {
    word_count_scalar_with_state(content, true, locale).counts
}

pub(crate) fn word_count_scalar_with_state(
    content: &[u8],
    initial_seen_space: bool,
    locale: LocaleEncoding,
) -> ScalarResult {
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
        chars: content.len(), // C locale: bytes = chars
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
        incomplete_bytes: 0, // C locale: no incomplete UTF-8
        seen_space,
    }
}

/// Count words/lines/chars in UTF-8 mode (iterate over characters, not bytes)
/// Handles invalid UTF-8 according to GNU wc semantics:
/// - Invalid bytes count as bytes but NOT as chars
/// - Invalid bytes are treated as non-whitespace (join words)
fn word_count_scalar_chars(content: &[u8], initial_seen_space: bool) -> ScalarResult {
    // Detect incomplete UTF-8 at end
    let incomplete = detect_incomplete_utf8_suffix(content);
    let process_len = content.len() - incomplete;

    let mut counts = FileCounts {
        lines: 0,
        words: 0,
        bytes: content.len(),
        chars: 0,
    };
    let mut seen_space = initial_seen_space;
    let mut i = 0;

    // Decode UTF-8 character by character, handling invalid sequences
    while i < process_len {
        match decode_utf8_char_at(&content[i..]) {
            Ok((ch, len)) => {
                // Valid UTF-8 character
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

                i += len;
            }
            Err(_) => {
                // Invalid UTF-8 byte - GNU wc semantics:
                // 1. Byte is already counted in counts.bytes
                // 2. NOT counted as a char (don't increment counts.chars)
                // 3. Treated as non-whitespace (joins words but doesn't start them)

                // Check if it's a newline byte (for line counting)
                if content[i] == b'\n' {
                    counts.lines += 1;
                    seen_space = true;
                } else {
                    // Invalid bytes are non-whitespace BUT don't start new words
                    // They only prevent word breaks (join adjacent words)
                    // So we do NOT increment words here, just clear seen_space if in a word
                    // This makes: "hello\xFFworld" → 1 word, but "\xFF\n" → 0 words
                    // seen_space stays as-is (invalid byte doesn't change word boundary state on its own)
                }

                i += 1;
            }
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
                return if bytes_from_here < 2 {
                    bytes_from_here
                } else {
                    0
                };
            }
            Utf8ByteType::Start3Byte => {
                // Need 3 bytes total
                return if bytes_from_here < 3 {
                    bytes_from_here
                } else {
                    0
                };
            }
            Utf8ByteType::Start4Byte => {
                // Need 4 bytes total
                return if bytes_from_here < 4 {
                    bytes_from_here
                } else {
                    0
                };
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

/// Decode a single UTF-8 character at the start of a byte slice.
/// Returns Ok((char, bytes_consumed)) on success, Err(()) on invalid UTF-8.
fn decode_utf8_char_at(bytes: &[u8]) -> Result<(char, usize), ()> {
    if bytes.is_empty() {
        return Err(());
    }

    let first = bytes[0];

    // ASCII fast path
    if first < 0x80 {
        return Ok((first as char, 1));
    }

    // Determine expected length from first byte
    let len = match first {
        0b1100_0000..=0b1101_1111 => 2,
        0b1110_0000..=0b1110_1111 => 3,
        0b1111_0000..=0b1111_0111 => 4,
        _ => return Err(()), // Invalid start byte
    };

    // Check we have enough bytes
    if bytes.len() < len {
        return Err(()); // Incomplete sequence
    }

    // Validate and decode using std::str
    match std::str::from_utf8(&bytes[..len]) {
        Ok(s) => {
            // Get the first (and only) character
            if let Some(ch) = s.chars().next() {
                Ok((ch, len))
            } else {
                Err(())
            }
        }
        Err(_) => Err(()), // Invalid sequence
    }
}
