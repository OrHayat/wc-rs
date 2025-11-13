// //! x86/x86_64 SIMD optimizations for text processing
// //!
// //! This module contains platform-specific optimizations using:
// //! - AVX2: 32 bytes/instruction (Intel Haswell+, AMD Excavator+)
// //! - SSE2: 16 bytes/instruction (almost all x86_64 CPUs)

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::arch::x86_64::*;

use crate::FileCounts;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::LocaleEncoding;
use crate::wc_default;

/// Internal SIMD results
#[derive(Debug, Clone, Copy)]
struct SimdCounts {
    lines: usize,
    words: usize,
    chars: usize,
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn count_text_simd(content: &str, locale: LocaleEncoding) -> Option<FileCounts> {
    if is_x86_feature_detected!("sse2") {
        let simd_result = unsafe { count_text_sse2(content.as_bytes(), locale) };
        return Some(FileCounts {
            lines: simd_result.lines,
            words: simd_result.words,
            bytes: content.len(),
            chars: simd_result.chars,
        });
    }
    None
}

// ============================================================================
// SSE2 Helper Functions - Reusable for AVX2/AVX-512
// ============================================================================

/// Count newlines in a SIMD chunk
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
#[inline]
unsafe fn sse2_count_newlines(chunk: __m128i) -> usize {
    let newline_vec = _mm_set1_epi8(b'\n' as i8);
    let cmp = _mm_cmpeq_epi8(chunk, newline_vec);
    let mask = _mm_movemask_epi8(cmp) as u16;
    mask.count_ones() as usize
}

/// Check if chunk contains non-ASCII bytes (> 0x7F)
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
#[inline]
unsafe fn sse2_has_non_ascii(chunk: __m128i) -> bool {
    let threshold = _mm_set1_epi8(0x7F as i8);
    let cmp = _mm_cmpgt_epi8(chunk, threshold);
    _mm_movemask_epi8(cmp) != 0
}

/// Count UTF-8 characters (non-continuation bytes)
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
#[inline]
unsafe fn sse2_count_utf8_chars(chunk: __m128i) -> usize {
    let cont_mask = _mm_set1_epi8(0b11000000u8 as i8);
    let cont_pattern = _mm_set1_epi8(0b10000000u8 as i8);

    let masked = _mm_and_si128(chunk, cont_mask);
    let is_continuation = _mm_cmpeq_epi8(masked, cont_pattern);
    let cont_bits = _mm_movemask_epi8(is_continuation) as u16;

    16 - cont_bits.count_ones() as usize
}

/// Detect ASCII whitespace: space (0x20) or range [0x09-0x0D]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
#[inline]
unsafe fn sse2_detect_whitespace(chunk: __m128i) -> u16 {
    let ws_min = _mm_set1_epi8(0x09u8 as i8); // tab
    let ws_max = _mm_set1_epi8(0x0Du8 as i8); // CR
    let space = _mm_set1_epi8(0x20u8 as i8);

    // Range check: [0x09, 0x0D]
    let ge_min = _mm_cmpgt_epi8(chunk, _mm_sub_epi8(ws_min, _mm_set1_epi8(1)));
    let le_max = _mm_cmpgt_epi8(_mm_add_epi8(ws_max, _mm_set1_epi8(1)), chunk);
    let in_range = _mm_and_si128(ge_min, le_max);

    // Check space
    let is_space = _mm_cmpeq_epi8(chunk, space);

    // Combine
    let is_ws = _mm_or_si128(in_range, is_space);
    _mm_movemask_epi8(is_ws) as u16
}

/// Count word starts from whitespace mask
/// A word start is: not_ws[i] && prev_was_ws[i-1]
#[inline]
fn count_word_starts_from_mask(ws_mask: u16, seen_space_before: bool) -> (usize, bool) {
    let not_ws = !ws_mask;

    // Shift LEFT by 1 to get "previous byte was whitespace"
    // For bit i: we want to know if bit i-1 was set
    // Fill LSB (bit 0) with seen_space_before state
    let prev_was_ws = (ws_mask << 1) | (if seen_space_before { 1 } else { 0 });

    // Word starts: current is not_ws AND previous was whitespace
    let word_starts = not_ws & prev_was_ws;
    let count = word_starts.count_ones() as usize;

    // Update: last byte is whitespace?
    let last_is_ws = (ws_mask & 0x8000) != 0;

    (count, last_is_ws)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn count_text_sse2(content: &[u8], locale: LocaleEncoding) -> SimdCounts {
    unsafe { count_text_sse2_manual(content, locale) }
}
// ============================================================================
// legacy SSE2 Implementation - kept for doc
// ============================================================================

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn count_text_sse2_manual(content: &[u8], locale: LocaleEncoding) -> SimdCounts {
    const CHUNK_SIZE: usize = 16;
    let mut counts = SimdCounts {
        lines: 0,
        words: 0,
        chars: 0,
    };
    let mut chunks = content.chunks_exact(CHUNK_SIZE);

    let mut seen_space = true;
    let mut carry: Vec<u8> = Vec::with_capacity(3);

    for chunk in chunks.by_ref() {
        let chunk_vec = unsafe { _mm_loadu_si128(chunk.as_ptr() as *const __m128i) };

        let has_non_ascii = unsafe { sse2_has_non_ascii(chunk_vec) };
        let has_carry = !carry.is_empty();

        // Non-ASCII UTF-8: fallback to scalar for Unicode whitespace
        if (has_non_ascii || has_carry) && locale == LocaleEncoding::Utf8 {
            let mut combined = carry.clone();
            combined.extend_from_slice(chunk);

            let result = wc_default::word_count_scalar_with_state(&combined, seen_space, locale);
            counts.lines += result.counts.lines;
            counts.chars += result.counts.chars;
            counts.words += result.counts.words;
            seen_space = result.seen_space;

            // Save incomplete UTF-8 sequence
            carry.clear();
            if result.incomplete_bytes > 0 {
                let start = combined.len() - result.incomplete_bytes;
                carry.extend_from_slice(&combined[start..]);
            }
        } else {
            // Fast SIMD path: C locale or pure ASCII
            counts.lines += unsafe { sse2_count_newlines(chunk_vec) };

            counts.chars += match locale {
                LocaleEncoding::C => CHUNK_SIZE,
                LocaleEncoding::Utf8 => unsafe { sse2_count_utf8_chars(chunk_vec) },
            };

            let ws_mask = unsafe { sse2_detect_whitespace(chunk_vec) };
            let (word_count, last_is_ws) = count_word_starts_from_mask(ws_mask, seen_space);
            counts.words += word_count;
            seen_space = last_is_ws;

            carry.clear();
        }
    }

    // Process remainder
    let remainder = chunks.remainder();
    if !remainder.is_empty() || !carry.is_empty() {
        let mut final_buf = carry;
        final_buf.extend_from_slice(remainder);

        let result = wc_default::word_count_scalar_with_state(&final_buf, seen_space, locale);
        counts.chars += result.counts.chars;
        counts.lines += result.counts.lines;
        counts.words += result.counts.words;
    }

    counts
}
