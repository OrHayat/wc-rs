


#[cfg(target_arch = "aarch64")]
use  std::arch::aarch64::*;
use crate::{FileCounts, LocaleEncoding};
use crate::wc_default;


/// Attempts SIMD-accelerated text counting on ARM64 processors.
///
/// Returns `Some(FileCounts)` if NEON instructions are available, `None` otherwise.
#[cfg(target_arch = "aarch64")]
pub fn count_text_simd(content: &[u8], locale: LocaleEncoding) -> Option<FileCounts> {
    if std::arch::is_aarch64_feature_detected!("neon") {
        return  Some(unsafe {
            count_text_neon(content, locale)
        })
    }
    None
}

/// SIMD vectors used for pattern matching in NEON instructions
struct SimdVectors {
    newline: uint8x16_t,
    ws_range_min: uint8x16_t,
    ws_range_max: uint8x16_t,
    space: uint8x16_t,
    ones: uint8x16_t,
    cont_mask: uint8x16_t,
    cont_pattern: uint8x16_t,
    ascii_threshold: uint8x16_t,
}

impl SimdVectors {
    unsafe fn new() -> Self {
        unsafe {
            Self {
                newline: vdupq_n_u8(b'\n'),
                ws_range_min: vdupq_n_u8(0x09),  // tab
                ws_range_max: vdupq_n_u8(0x0D),  // carriage return
                space: vdupq_n_u8(0x20),
                ones: vdupq_n_u8(1),
                cont_mask: vdupq_n_u8(0b11000000),     // Mask to check top 2 bits
                cont_pattern: vdupq_n_u8(0b10000000),  // UTF-8 continuation bytes: 0b10xxxxxx
                ascii_threshold: vdupq_n_u8(0x80),
            }
        }
    }
}

/// Result from processing a single chunk with SIMD
struct ChunkCounts {
    lines: usize,
    words: usize,
    chars: usize,
    /// Whether the last character in the chunk was whitespace
    last_was_space: bool,
}

/// Result from processing UTF-8 content with scalar fallback
struct Utf8ProcessResult {
    lines: usize,
    words: usize,
    chars: usize,
    seen_space: bool,
}

/// Processes UTF-8 chunk with non-ASCII characters using scalar fallback.
/// This handles Unicode whitespace correctly by falling back to the scalar implementation.
///
/// The carry buffer is modified in place and reused for efficiency.
fn process_chunk_utf8_scalar(
    chunk: &[u8],
    carry_buffer: &mut Vec<u8>,
    seen_space: bool,
    locale: LocaleEncoding,
) -> Utf8ProcessResult {
    // Combine carry bytes with current chunk (reusing buffer)
    carry_buffer.extend_from_slice(chunk);

    let scalar_result = wc_default::word_count_scalar_with_state(carry_buffer, seen_space, locale);

    // Update carry buffer with incomplete bytes for next chunk
    if scalar_result.incomplete_bytes > 0 {
        let start = carry_buffer.len() - scalar_result.incomplete_bytes;
        // Move incomplete bytes to the front
        carry_buffer.copy_within(start.., 0);
        carry_buffer.truncate(scalar_result.incomplete_bytes);
    } else {
        carry_buffer.clear();
    }

    Utf8ProcessResult {
        lines: scalar_result.counts.lines,
        words: scalar_result.counts.words,
        chars: scalar_result.counts.chars,
        seen_space: scalar_result.seen_space,
    }
}

/// Processes a single 16-byte chunk using SIMD for ASCII or C locale.
/// This is the fast path that uses NEON instructions for counting.
unsafe fn process_chunk_simd(
    chunk_vec: uint8x16_t,
    simd: &SimdVectors,
    seen_space: bool,
    locale: LocaleEncoding,
) -> ChunkCounts {
    unsafe {
        const CHUNK_SIZE: usize = 16;

        // Count newlines
        let newline_cmp = vceqq_u8(chunk_vec, simd.newline);
        let mask = vandq_u8(newline_cmp, simd.ones);
        let lines = vaddvq_u8(mask) as usize;

        // Count characters based on locale
        let chars = match locale {
            LocaleEncoding::C => {
                // C locale: every byte is a character
                CHUNK_SIZE
            }
            LocaleEncoding::Utf8 => {
                // UTF-8 locale: skip continuation bytes (0b10xxxxxx)
                let masked = vandq_u8(chunk_vec, simd.cont_mask);
                let is_continuation = vceqq_u8(masked, simd.cont_pattern);
                let is_not_continuation = vmvnq_u8(is_continuation);
                let mask = vandq_u8(is_not_continuation, simd.ones);
                vaddvq_u8(mask) as usize
            }
        };

        // Count words - ASCII whitespace only
        let in_range = vandq_u8(
            vcgeq_u8(chunk_vec, simd.ws_range_min),
            vcleq_u8(chunk_vec, simd.ws_range_max),
        );
        let is_space = vceqq_u8(chunk_vec, simd.space);
        let is_ws = vorrq_u8(in_range, is_space);
        let is_not_ws = vmvnq_u8(is_ws);

        // Create "previous byte" vector by shifting
        let prev_byte_val = if seen_space { 0x00u8 } else { 0xFFu8 };
        let prev_vec = vdupq_n_u8(prev_byte_val);
        let prev_is_not_ws = vextq_u8(prev_vec, is_not_ws, 15);

        // Find word starts
        let prev_is_ws = vmvnq_u8(prev_is_not_ws);
        let word_starts = vandq_u8(is_not_ws, prev_is_ws);
        let mask = vandq_u8(word_starts, simd.ones);
        let words = vaddvq_u8(mask) as usize;

        // Check if the last byte was whitespace (for next chunk)
        let mut last_bytes = [0u8; 16];
        vst1q_u8(last_bytes.as_mut_ptr(), is_not_ws);
        let last_was_space = last_bytes[15] == 0x00;

        ChunkCounts {
            lines,
            words,
            chars,
            last_was_space,
        }
    }
}

/// Counts lines, words, and UTF-8 characters using ARM NEON SIMD instructions.
///
/// # Strategy
/// - For pure ASCII or C locale: uses fast SIMD processing
/// - For UTF-8 with multi-byte chars: falls back to scalar processing for correct Unicode handling
/// - Processes input in 16-byte chunks (NEON vector size)
/// - Handles incomplete UTF-8 sequences at chunk boundaries with a carry buffer
unsafe fn count_text_neon(content: &[u8], locale: LocaleEncoding) -> FileCounts {
    unsafe {
        let mut counts = FileCounts {
            lines: 0,
            words: 0,
            chars: 0,
            bytes: content.len(),
        };

        const CHUNK_SIZE: usize = 16; // NEON processes 128 bits (16 bytes) at a time
        let mut chunks = content.chunks_exact(CHUNK_SIZE);
        let simd = SimdVectors::new();

        // Track whether the previous character was whitespace (for word counting)
        let mut seen_space = true;

        // Carry buffer for incomplete UTF-8 sequences at chunk boundaries
        let mut utf8_carry_buffer: Vec<u8> = Vec::with_capacity(3);

        // Process each 16-byte chunk
        for chunk in chunks.by_ref() {
            let chunk_vec: uint8x16_t = vld1q_u8(chunk.as_ptr());

            // Detect non-ASCII bytes in the chunk
            let has_non_ascii_mask = vcgeq_u8(chunk_vec, simd.ascii_threshold);
            let non_ascii_count = vaddvq_u8(vandq_u8(has_non_ascii_mask, simd.ones)) as usize;
            let has_pending_utf8_bytes = !utf8_carry_buffer.is_empty();

            // Choose processing path based on content
            let needs_utf8_handling = (non_ascii_count > 0 || has_pending_utf8_bytes)
                && locale == LocaleEncoding::Utf8;

            if needs_utf8_handling {
                // Slow path: UTF-8 with multi-byte sequences
                // Use scalar processing for correct Unicode whitespace handling
                let result = process_chunk_utf8_scalar(
                    chunk,
                    &mut utf8_carry_buffer,
                    seen_space,
                    locale
                );

                counts.lines += result.lines;
                counts.chars += result.chars;
                counts.words += result.words;
                seen_space = result.seen_space;
                // utf8_carry_buffer is modified in place
            } else {
                // Fast path: Pure ASCII or C locale
                // Use SIMD for maximum performance
                let result = process_chunk_simd(chunk_vec, &simd, seen_space, locale);
                counts.lines += result.lines;
                counts.chars += result.chars;
                counts.words += result.words;
                seen_space = result.last_was_space;
                // C locale/ASCII doesn't need carry buffer
                utf8_carry_buffer.clear();
            }
        }

        // Process remainder bytes (less than 16 bytes) with any carry bytes
        let remainder = chunks.remainder();

        // Reuse the carry buffer for the remainder
        utf8_carry_buffer.extend_from_slice(remainder);

        if !utf8_carry_buffer.is_empty() {
            let result = wc_default::word_count_scalar_with_state(
                &utf8_carry_buffer,
                seen_space,
                locale
            );

            counts.lines += result.counts.lines;
            counts.words += result.counts.words;
            counts.chars += result.counts.chars;
            // Note: incomplete_bytes at EOF are ignored (partial character at end of file)
        }

        counts
    }
}



#[doc(hidden)]
#[allow(dead_code)]
fn print_u8x16(v: uint8x16_t, name: &str) {
    let mut arr = [0u8; 16];
    unsafe { vst1q_u8(arr.as_mut_ptr(), v) }
    println!("{} = {:?}", name, arr);
}

#[doc(hidden)]
#[allow(dead_code)]
fn print_u16x8(v: uint16x8_t, name: &str) {
    let mut arr = [0u16; 8];
    unsafe { vst1q_u16(arr.as_mut_ptr(), v) }
    println!("{} = {:?}", name, arr);
}

#[doc(hidden)]
#[allow(dead_code)]
fn print_u32x4(v: uint32x4_t, name: &str) {
    let mut arr = [0u32; 4];
    unsafe { vst1q_u32(arr.as_mut_ptr(), v) }
    println!("{} = {:?}", name, arr);
}

#[doc(hidden)]
#[allow(dead_code)]
fn print_u64x2(v: uint64x2_t, name: &str) {
    let mut arr = [0u64; 2];
    unsafe { vst1q_u64(arr.as_mut_ptr(), v) }
    println!("{} = {:?}", name, arr);
}
