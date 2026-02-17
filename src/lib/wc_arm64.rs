#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use crate::wc_default;
use crate::{FileCounts, LocaleEncoding};

// ============================================================================
// SVE FFI Declarations (C implementation)
// ============================================================================
// Only include SVE FFI if the C library was successfully built

#[cfg(sve_available)]
mod sve_ffi {
    use crate::FileCounts;

    // Match C's FileCountsResult struct
    #[repr(C)]
    pub struct FileCountsResult {
        pub counts: FileCounts,
        pub success: bool,
    }

    #[link(name = "wc_arm64_sve", kind = "static")]
    unsafe extern "C" {
        // Checked version: safe, verifies CPU supports SVE
        #[allow(dead_code)]
        pub fn count_text_sve_c_checked(
            content: *const u8,
            len: usize,
            locale: u8,
        ) -> FileCountsResult;

        // Unchecked version: assumes SVE is available
        pub fn count_text_sve_c_unchecked(content: *const u8, len: usize, locale: u8)
        -> FileCounts;
    }
}

#[cfg(sve_available)]
pub(crate) unsafe fn count_text_sve(content: &[u8], locale: LocaleEncoding) -> FileCounts {
    let locale_byte = match locale {
        LocaleEncoding::SingleByte => 0,
        LocaleEncoding::Utf8 => 1,
    };

    unsafe { sve_ffi::count_text_sve_c_unchecked(content.as_ptr(), content.len(), locale_byte) }
}

// ============================================================================
// NEON Helper Functions - Reusable operations
// ============================================================================

/// Count newlines in a NEON chunk
#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn neon_count_newlines(chunk: uint8x16_t) -> usize {
    unsafe {
        let newline_vec = vdupq_n_u8(b'\n');
        let newline_cmp = vceqq_u8(chunk, newline_vec);
        let ones = vdupq_n_u8(1);
        let mask = vandq_u8(newline_cmp, ones);
        vaddvq_u8(mask) as usize
    }
}

/// Check if chunk contains non-ASCII bytes (>= 0x80)
#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn neon_has_non_ascii(chunk: uint8x16_t) -> bool {
    unsafe {
        let ascii_threshold = vdupq_n_u8(0x80);
        let has_non_ascii_mask = vcgeq_u8(chunk, ascii_threshold);
        let ones = vdupq_n_u8(1);
        let non_ascii_count = vaddvq_u8(vandq_u8(has_non_ascii_mask, ones));
        non_ascii_count > 0
    }
}

/// Count UTF-8 characters (non-continuation bytes)
#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn neon_count_utf8_chars(chunk: uint8x16_t) -> usize {
    unsafe {
        let cont_mask = vdupq_n_u8(0b11000000);
        let cont_pattern = vdupq_n_u8(0b10000000);

        let masked = vandq_u8(chunk, cont_mask);
        let is_continuation = vceqq_u8(masked, cont_pattern);
        let is_not_continuation = vmvnq_u8(is_continuation);
        let ones = vdupq_n_u8(1);
        let mask = vandq_u8(is_not_continuation, ones);
        vaddvq_u8(mask) as usize
    }
}

/// Detect ASCII whitespace: space (0x20) or range [0x09-0x0D]
/// Returns a 16-bit mask where each bit represents whether that byte is whitespace
#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn neon_detect_whitespace(chunk: uint8x16_t) -> u16 {
    unsafe {
        let ws_min = vdupq_n_u8(0x09); // tab
        let ws_max = vdupq_n_u8(0x0D); // carriage return
        let space = vdupq_n_u8(0x20);

        // Range check: [0x09, 0x0D]
        let in_range = vandq_u8(vcgeq_u8(chunk, ws_min), vcleq_u8(chunk, ws_max));

        // Check space
        let is_space = vceqq_u8(chunk, space);

        // Combine
        let is_ws = vorrq_u8(in_range, is_space);

        // Convert to bitmask
        let mut bytes = [0u8; 16];
        vst1q_u8(bytes.as_mut_ptr(), is_ws);

        let mut mask = 0u16;
        for (i, &byte) in bytes.iter().enumerate() {
            if byte == 0xFF {
                mask |= 1 << i;
            }
        }
        mask
    }
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

/// Process data using scalar fallback, handling UTF-8 carry buffer
///
/// Combines carry buffer with new data, processes using scalar word counting,
/// and updates the carry buffer with any incomplete UTF-8 sequences.
///
/// Returns the new `seen_space` state for word counting.
#[inline]
fn process_scalar_with_carry(
    new_data: &[u8],
    carry: &mut Vec<u8>,
    counts: &mut FileCounts,
    seen_space: bool,
    locale: LocaleEncoding,
) -> bool {
    carry.extend_from_slice(new_data);

    let result = wc_default::word_count_scalar_with_state(carry, seen_space, locale);

    counts.lines += result.counts.lines;
    counts.chars += result.counts.chars;
    counts.words += result.counts.words;

    // Update carry buffer with incomplete UTF-8 sequences
    if result.incomplete_bytes > 0 {
        let start = carry.len() - result.incomplete_bytes;
        carry.copy_within(start.., 0);
        carry.truncate(result.incomplete_bytes);
    } else {
        carry.clear();
    }

    result.seen_space
}

/// NEON implementation - manually written to match SSE2 pattern
#[cfg(target_arch = "aarch64")]
pub(crate) unsafe fn count_text_neon(content: &[u8], locale: LocaleEncoding) -> FileCounts {
    const CHUNK_SIZE: usize = 16;
    let mut result_acc = FileCounts {
        lines: 0,
        words: 0,
        chars: 0,
        bytes: content.len(),
    };
    let mut chunks = content.chunks_exact(CHUNK_SIZE);

    // Word counting state: tracks if previous byte was whitespace
    let mut seen_space = true;

    // UTF-8 carry buffer: incomplete multi-byte sequences at chunk boundaries
    let mut carry: Vec<u8> = Vec::with_capacity(4);

    for chunk in chunks.by_ref() {
        let chunk_vec = unsafe { vld1q_u8(chunk.as_ptr()) };

        let has_non_ascii = unsafe { neon_has_non_ascii(chunk_vec) };
        let has_carry = !carry.is_empty();

        // Choose processing path based on content and locale
        // Scalar path: UTF-8 locale with non-ASCII or incomplete sequences
        // SIMD path: C locale (any bytes) OR pure ASCII UTF-8
        if (has_non_ascii || has_carry) && locale == LocaleEncoding::Utf8 {
            seen_space =
                process_scalar_with_carry(chunk, &mut carry, &mut result_acc, seen_space, locale);
        } else {
            // Fast SIMD path: C locale or pure ASCII
            result_acc.lines += unsafe { neon_count_newlines(chunk_vec) };

            result_acc.chars += match locale {
                LocaleEncoding::SingleByte => CHUNK_SIZE,
                LocaleEncoding::Utf8 => unsafe { neon_count_utf8_chars(chunk_vec) },
            };

            let ws_mask = unsafe { neon_detect_whitespace(chunk_vec) };
            let (word_count, last_is_ws) = count_word_starts_from_mask(ws_mask, seen_space);
            result_acc.words += word_count;
            seen_space = last_is_ws;

            carry.clear();
        }
    }

    // Process remainder
    let remainder = chunks.remainder();
    if !remainder.is_empty() || !carry.is_empty() {
        process_scalar_with_carry(remainder, &mut carry, &mut result_acc, seen_space, locale);
    }

    result_acc
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
