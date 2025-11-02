//! x86/x86_64 SIMD optimizations for text processing
//!
//! This module contains platform-specific optimizations using:
//! - AVX2: 32 bytes/instruction (Intel Haswell+, AMD Excavator+)

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// Import FileCounts from parent module
use crate::FileCounts;

/// Internal SIMD results
#[derive(Debug, Clone, Copy)]
struct SimdCounts {
    lines: usize,
    words: usize,
    chars: usize,
}

/// Try to count using SIMD - returns None if SIMD not available
pub fn count_text_simd(content: &[u8]) -> Option<FileCounts> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            let simd_result = unsafe { count_text_avx2(content) };
            return Some(FileCounts {
                lines: simd_result.lines,
                words: simd_result.words,
                bytes: content.len(),
                chars: simd_result.chars,
            });
        }
    }

    // No SIMD available
    None
}

/// Helper function to check if a byte is ASCII whitespace
fn is_ascii_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn count_text_avx2(content: &[u8]) -> SimdCounts {
    let mut lines = 0;
    let mut words = 0;
    let mut chars = 0;

    // SIMD vectors for comparison
    // SIMD works by processing multiple bytes in parallel. AVX2 can process 32 bytes at once.
    // We create "pattern vectors" filled with the same byte value to compare against.

    // _mm256_set1_epi8: Creates a 256-bit vector (32 bytes) where every byte is the same value
    // This lets us compare all 32 bytes in a chunk against '\n' simultaneously
    let newline_vec = _mm256_set1_epi8(b'\n' as i8); // ['\n', '\n', '\n', ... 32 times]
    let space_vec = _mm256_set1_epi8(b' ' as i8); // [' ', ' ', ' ', ... 32 times]
    let tab_vec = _mm256_set1_epi8(b'\t' as i8); // ['\t', '\t', '\t', ... 32 times]
    let cr_vec = _mm256_set1_epi8(b'\r' as i8); // ['\r', '\r', '\r', ... 32 times]

    // For UTF-8 character counting: detect continuation bytes (10xxxxxx)
    // UTF-8 continuation bytes are 0x80-0xBF (binary: 10000000 to 10111111)
    // To detect them, we mask the top 2 bits and check if they equal 10xxxxxx
    let utf8_cont_mask = _mm256_set1_epi8(0b11000000u8 as i8); // Mask to isolate top 2 bits
    let utf8_cont_pattern = _mm256_set1_epi8(0b10000000u8 as i8); // Pattern 10xxxxxx

    let chunks = content.len() / 32; // How many 32-byte chunks we can process
    let mut i = 0;
    let mut prev_was_whitespace = true; // Assume we start after whitespace for word counting

    // Process 32-byte chunks with AVX2
    for _ in 0..chunks {
        unsafe {
            // Load 32 bytes from memory into a SIMD register
            // _mm256_loadu_si256: Loads 256 bits (32 bytes) from unaligned memory
            // "unaligned" means the data doesn't have to be at a special memory address
            let chunk = _mm256_loadu_si256(content.as_ptr().add(i) as *const __m256i);

            // Count newlines: Compare each byte in chunk with '\n'
            // _mm256_cmpeq_epi8: Compares 32 bytes simultaneously
            // Returns a vector where each byte is 0xFF if equal, 0x00 if not equal
            let newline_cmp = _mm256_cmpeq_epi8(chunk, newline_vec);

            // _mm256_movemask_epi8: Extracts the high bit of each byte into a 32-bit mask
            // Since 0xFF has high bit = 1 and 0x00 has high bit = 0, this gives us a bitmask
            // where bit N is 1 if byte N was a newline
            let newline_mask = _mm256_movemask_epi8(newline_cmp) as u32;

            // count_ones(): Counts how many bits are set in the mask = number of newlines found
            lines += newline_mask.count_ones() as usize;

            // Count UTF-8 characters by counting non-continuation bytes
            // Strategy: UTF-8 continuation bytes have pattern 10xxxxxx
            // We mask each byte with 11000000 to get the top 2 bits, then check if they equal 10000000

            // _mm256_and_si256: Bitwise AND operation on 32 bytes simultaneously
            // This masks out the bottom 6 bits, keeping only the top 2 bits
            let masked_chunk = _mm256_and_si256(chunk, utf8_cont_mask);

            // Compare the masked bytes with the continuation pattern (10000000)
            let is_continuation = _mm256_cmpeq_epi8(masked_chunk, utf8_cont_pattern);
            let continuation_mask = _mm256_movemask_epi8(is_continuation) as u32;

            // Count non-continuation bytes = UTF-8 character count
            // Each UTF-8 character starts with a non-continuation byte
            chars += 32 - continuation_mask.count_ones() as usize;

            // Detect whitespace for word counting (only ASCII whitespace)
            // We check for space, tab, carriage return, and newline simultaneously
            let space_cmp = _mm256_cmpeq_epi8(chunk, space_vec);
            let tab_cmp = _mm256_cmpeq_epi8(chunk, tab_vec);
            let cr_cmp = _mm256_cmpeq_epi8(chunk, cr_vec);
            let newline_cmp_for_words = _mm256_cmpeq_epi8(chunk, newline_vec);

            // Combine all whitespace comparisons using OR operations
            // _mm256_or_si256: Bitwise OR operation on 32 bytes simultaneously
            // This combines multiple comparison results into one
            let ws1 = _mm256_or_si256(space_cmp, tab_cmp); // space OR tab
            let ws2 = _mm256_or_si256(cr_cmp, newline_cmp_for_words); // CR OR newline
            let whitespace_mask = _mm256_movemask_epi8(_mm256_or_si256(ws1, ws2)) as u32;

            // Count word transitions (whitespace to non-whitespace)
            // We iterate through each bit in the mask to track word boundaries
            for bit_idx in 0..32 {
                // Check if this byte position is whitespace
                let is_whitespace = (whitespace_mask & (1 << bit_idx)) != 0;

                // A new word starts when we transition from whitespace to non-whitespace
                if prev_was_whitespace && !is_whitespace {
                    words += 1;
                }
                prev_was_whitespace = is_whitespace;
            }
        }

        i += 32; // Move to the next 32-byte chunk
    }

    // Handle remaining bytes with scalar processing
    // After processing all complete 32-byte chunks, we may have some leftover bytes
    // (e.g., if file is 100 bytes, we process 96 bytes with SIMD, 4 bytes with scalar)
    let scalar_result = count_text_scalar(&content[i..]);

    // Adjust for transition from SIMD to scalar processing
    // We need to be careful not to double-count words that span the SIMD/scalar boundary
    if i > 0 && i < content.len() && !prev_was_whitespace {
        // We ended SIMD processing in the middle of a word
        let first_remaining_byte = content[i];
        if !is_ascii_whitespace(first_remaining_byte) {
            // The first scalar byte continues the word from SIMD, so don't double-count
            words += scalar_result.1.saturating_sub(1);
        } else {
            // The first scalar byte is whitespace, so it ends the SIMD word
            words += scalar_result.1;
        }
    } else {
        // No boundary issues - just add the scalar word count
        words += scalar_result.1;
    }

    // Add the scalar results for lines and characters
    lines += scalar_result.0; // lines from remaining bytes
    chars += scalar_result.2; // characters from remaining bytes

    SimdCounts {
        lines,
        words,
        chars,
    }
}

fn count_text_scalar(content: &[u8]) -> (usize, usize, usize) {
    let mut lines = 0;
    let mut words = 0;
    let mut chars = 0;
    let mut in_word = false;

    for &byte in content {
        if byte == b'\n' {
            lines += 1;
        }

        // Count UTF-8 characters: count bytes that are NOT continuation bytes
        // UTF-8 continuation bytes have pattern 10xxxxxx
        if (byte & 0b11000000) != 0b10000000 {
            chars += 1;
        }

        if is_ascii_whitespace(byte) {
            in_word = false;
        } else if !in_word {
            words += 1;
            in_word = true;
        }
    }

    // wc behavior: count final line even if it doesn't end with newline
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    (lines, words, chars)
}
