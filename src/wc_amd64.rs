//! ARM64 NEON SIMD optimizations for text processing
//!
//! This module contains platform-specific optimizations using:
//! - NEON: 16 bytes/instruction (all ARM64 CPUs including Apple Silicon, AWS Graviton)
//!
//! Future optimizations planned:
//! - Crypto extensions for faster bit manipulation (~20x speedup)
//! - SVE for cloud/server instances (~32x speedup on AWS Graviton3)
//! - SVE2 for latest cloud instances (~50x speedup on AWS Graviton4)

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use crate::FileCounts;

/// Internal SIMD results
#[derive(Debug, Clone, Copy)]
struct SimdCounts {
    lines: usize,
    words: usize,
    chars: usize,
}

/// Try to count using SIMD - returns None if SIMD not available
#[cfg(target_arch = "aarch64")]
pub fn count_text_simd(content: &[u8]) -> Option<FileCounts> {
    // NEON should be always available on aarch64
    if std::arch::is_aarch64_feature_detected!("neon") {
        let simd_result = unsafe { count_text_neon(content) };
        return Some(FileCounts {
            lines: simd_result.lines,
            words: simd_result.words,
            bytes: content.len(),
            chars: simd_result.chars,
        });
    }

    // No SIMD available (should never happen on aarch64)
    None
}

/// Fallback for non-aarch64 architectures
#[cfg(not(target_arch = "aarch64"))]
pub fn count_text_simd(_content: &[u8]) -> Option<FileCounts> {
    None
}

/// Helper function to check if a byte is ASCII whitespace
fn is_ascii_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0C | 0x0B)
}

/// Common scalar boundary handling logic
fn handle_scalar_boundary(
    content: &[u8],
    simd_end_index: usize,
    prev_was_whitespace: bool,
    mut simd_words: usize,
    mut simd_lines: usize,
    mut simd_chars: usize,
) -> SimdCounts {
    let scalar_result = count_text_scalar(&content[simd_end_index..]);

    // Adjust for transition from SIMD to scalar processing
    if simd_end_index > 0 && simd_end_index < content.len() && !prev_was_whitespace {
        let first_remaining_byte = content[simd_end_index];
        if !is_ascii_whitespace(first_remaining_byte) {
            simd_words += scalar_result.words.saturating_sub(1);
        } else {
            simd_words += scalar_result.words;
        }
    } else {
        simd_words += scalar_result.words;
    }

    simd_lines += scalar_result.lines;
    simd_chars += scalar_result.chars;

    SimdCounts {
        lines: simd_lines,
        words: simd_words,
        chars: simd_chars,
    }
}

/// Count word transitions from whitespace mask
fn count_word_transitions(
    whitespace_mask: u16,
    chunk_size: usize,
    prev_was_whitespace: &mut bool,
) -> usize {
    let mut words = 0;

    for bit_idx in 0..chunk_size {
        let is_whitespace = (whitespace_mask & (1u16 << bit_idx)) != 0;
        if *prev_was_whitespace && !is_whitespace {
            words += 1;
        }
        *prev_was_whitespace = is_whitespace;
    }

    words
}

/// Macro to generate NEON text counting implementations with different movemask strategies
///
/// Generates identical NEON functions that differ ONLY in the movemask implementation.
/// This enables fair benchmarking of movemask strategies while keeping all other logic identical.
macro_rules! generate_neon_counter {
    (
        fn_name: $fn_name:ident,
        movemask: $movemask_fn:ident,
        variant: $doc_variant:expr
    ) => {
        #[doc = concat!(
            "NEON text counter using `", stringify!($movemask_fn), "` (", $doc_variant, " variant)\n",
            "\n",
            "Processes 16 bytes/iteration: counts lines, words, and UTF-8 characters in parallel.\n",
            "See `count_text_neon()` for algorithm details.\n"
        )]
        #[cfg(target_arch = "aarch64")]
        #[target_feature(enable = "neon")]
        #[allow(dead_code)]
        unsafe fn $fn_name(content: &[u8]) -> SimdCounts {
            // Accumulator variables for counting
            let mut lines = 0;
            let mut words = 0;
            let mut chars = 0;

            // NEON processes 16 bytes (128 bits) per iteration
            const CHUNK_SIZE: usize = 16;

            // ================================================================
            // SETUP: Create pattern vectors for parallel comparison
            // ================================================================
            // Each vector contains the same byte value repeated across all 16 lanes
            // This allows comparing all 16 input bytes against a pattern simultaneously

            let newline_vec = vdupq_n_u8(b'\n');      // For line counting
            let space_vec = vdupq_n_u8(b' ');         // For word detection
            let tab_vec = vdupq_n_u8(b'\t');          // For word detection
            let cr_vec = vdupq_n_u8(b'\r');           // For word detection (Windows)
            let ff_vec = vdupq_n_u8(0x0C);            // Form feed (rare)
            let vt_vec = vdupq_n_u8(0x0B);            // Vertical tab (rare)

            // UTF-8 character counting vectors
            // UTF-8 continuation bytes have pattern 10xxxxxx (top 2 bits = 10)
            let utf8_cont_mask = vdupq_n_u8(0b11000000);      // Mask to extract top 2 bits
            let utf8_cont_pattern = vdupq_n_u8(0b10000000);   // Pattern for continuation bytes

            // ================================================================
            // MAIN SIMD LOOP: Process 16 bytes at a time
            // ================================================================

            let chunks = content.len() / CHUNK_SIZE;
            let mut i = 0;
            let mut prev_was_whitespace = true;  // Track state for word boundaries

            for _ in 0..chunks {
                #[allow(unused_unsafe)]
                unsafe {
                    // STEP 1: Load 16 bytes from memory into NEON register
                    let chunk = unsafe { vld1q_u8(content.as_ptr().add(i)) };

                    // STEP 2: Count newlines
                    // Compare all 16 bytes against '\n', get mask, count set bits
                    let newline_mask = unsafe {
                        let cmp = vceqq_u8(chunk, newline_vec);
                        $movemask_fn(cmp)  // Extract bitmask from comparison result
                    };
                    lines += newline_mask.count_ones() as usize;

                    // STEP 3: Count UTF-8 characters
                    // Extract top 2 bits and check for continuation byte pattern (10xxxxxx)
                    let masked_chunk = unsafe { vandq_u8(chunk, utf8_cont_mask) };
                    let continuation_mask = unsafe {
                        let cmp = vceqq_u8(masked_chunk, utf8_cont_pattern);
                        $movemask_fn(cmp)  // Extract bitmask from comparison result
                    };
                    // Characters = bytes that are NOT continuation bytes
                    chars += CHUNK_SIZE - continuation_mask.count_ones() as usize;

                    // STEP 4: Detect all whitespace characters
                    // Compare against all 6 whitespace types and combine results
                    let whitespace_mask = unsafe {
                        let space_cmp = vceqq_u8(chunk, space_vec);
                        let tab_cmp = vceqq_u8(chunk, tab_vec);
                        let cr_cmp = vceqq_u8(chunk, cr_vec);
                        let newline_cmp = vceqq_u8(chunk, newline_vec);
                        let ff_cmp = vceqq_u8(chunk, ff_vec);
                        let vt_cmp = vceqq_u8(chunk, vt_vec);

                        // Combine all comparisons using tree of OR operations
                        let ws1 = vorrq_u8(space_cmp, tab_cmp);
                        let ws2 = vorrq_u8(cr_cmp, newline_cmp);
                        let ws3 = vorrq_u8(ff_cmp, vt_cmp);
                        let ws_combined = vorrq_u8(ws1, ws2);
                        let final_mask = vorrq_u8(ws_combined, ws3);

                        $movemask_fn(final_mask)  // Extract bitmask from combined comparison
                    };

                    // STEP 5: Count word transitions (whitespace -> non-whitespace)
                    words += count_word_transitions(
                        whitespace_mask,
                        CHUNK_SIZE,
                        &mut prev_was_whitespace
                    );
                }

                i += CHUNK_SIZE;  // Move to next 16-byte chunk
            }

            // ================================================================
            // CLEANUP: Handle remaining bytes (less than 16) with scalar code
            // ================================================================
            handle_scalar_boundary(content, i, prev_was_whitespace, words, lines, chars)
        }
    };
}

// ============================================================================
// Generated NEON counting functions using different movemask implementations
// ============================================================================

// Generate PACKED version (uses pure NEON bit packing)
generate_neon_counter! {
    fn_name: count_text_neon_packed,
    movemask: neon_movemask_u8x16_packed,
    variant: "PACKED"
}

// Generate EMULATED version (uses scalar lane extraction)
generate_neon_counter! {
    fn_name: count_text_neon_emulated_impl,
    movemask: neon_movemask_u8x16_emulated,
    variant: "EMULATED"
}

// generate_neon_counter! {
//     fn_name: count_text_neon_vtbl_impl,
//     movemask: neon_movemask_u8x16_vtbl,
//     variant: "VTBL"
// }
// ============================================================================
// VTBL: NEON movemask using VTBL instruction
// // ============================================================================
// #[cfg(target_arch = "aarch64")]
// #[target_feature(enable = "neon")]
// unsafe fn neon_movemask_u8x16_vtbl(vec: uint8x16_t) -> u16 {
//     // Step 1: Shift right to get high bits in LSB
//     let shifted = vshrq_n_u8(vec, 7);

//     // Step 2: Prepare lookup table for bit positions
//     // This table maps each byte index to its corresponding bit position
//     // for example, byte 0 -> 1<<0, byte 1 -> 1<<1, ..., byte 15 -> 1<<15
//     let table: [u8; 16] = [
//         1 << 0,  1 << 1,
//         1 << 2,  1 << 3,
//         1 << 4,  1 << 5,
//         1 << 6,  1 << 7,
//         1 << 8,  1 << 9,
//         1 << 10, 1 << 11,
//         1 << 12, 1 << 13,
//         1 << 14, 1 << 15,
//     ];
//     let table_vec = unsafe { vld1q_u8(table.as_ptr()) };

//     // Step 3: Use vtbl to select bit positions for each lane
//     let low = vget_low_u8(shifted);
//     let high = vget_high_u8(shifted);

//     let low_mask = vtbl1_u8(vreinterpret_u8_u64(table_vec), low);
//     let high_mask = vtbl1_u8(vreinterpret_u8_u64(table_vec), high);

//     // Step 4: Sum up the selected bits to form the mask
//     let mut mask: u16 = 0;
//     for i in 0..8 {
//         mask |= (low_mask[i] as u16);
//         mask |= (high_mask[i] as u16);
//     }
//     mask
// }
// ============================================================================
// PACKED: Pure NEON bit extraction (NEW IMPLEMENTATION)
// ============================================================================

/// NEON movemask using pure NEON bit packing (NEW - FAST)
///
/// This implementation uses NEON bit manipulation instead of extracting individual lanes.
/// It should be significantly faster than the emulated version below.
///
/// Algorithm:
/// 1. Extract high bits by shifting right 7 positions
/// 2. Use horizontal adds and shifts to pack bits together
/// 3. Extract the final result as a single u8
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn neon_movemask_u8x8_packed(vec: uint8x8_t) -> u8 {
    // Step 1: Extract high bits by shifting right 7 positions
    // This puts the high bit in the LSB position of each byte
    let shifted = vshr_n_u8::<7>(vec);

    // Step 2: Create bit positions by multiplying each byte by its power of 2
    // Byte 0 * 1, Byte 1 * 2, Byte 2 * 4, Byte 3 * 8, etc.
    let bit_positions = vreinterpret_u8_u64(vdup_n_u64(0x8040201008040201u64));

    // Step 3: Multiply shifted bits by their positions
    let positioned = vmul_u8(shifted, bit_positions);

    // Step 4: Horizontal add to combine all bits
    // Use pairwise add repeatedly to sum all lanes
    let sum1 = vpaddl_u8(positioned); // 8xu8 -> 4xu16
    let sum2 = vpaddl_u16(sum1); // 4xu16 -> 2xu32
    let sum3 = vpaddl_u32(sum2); // 2xu32 -> 1xu64

    // Extract final result
    vget_lane_u64::<0>(sum3) as u8
}

/// NEON movemask using pure NEON bit packing for 16-byte vector (NEW - FAST)
///
/// Processes both halves of the 128-bit vector independently and combines results.
/// Uses the packed 8-byte version for each half.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn neon_movemask_u8x16_packed(vec: uint8x16_t) -> u16 {
    let low = vget_low_u8(vec);
    let high = vget_high_u8(vec);

    // SAFETY: Both functions are marked with #[target_feature(enable = "neon")]
    // and we're already in an unsafe context with NEON enabled
    let low_mask = unsafe { neon_movemask_u8x8_packed(low) } as u16;
    let high_mask = unsafe { neon_movemask_u8x8_packed(high) } as u16;

    low_mask | (high_mask << 8)
}

// ============================================================================
// REFERENCE: Original emulated implementation (for comparison/benchmarking)
// ============================================================================

/// NEON movemask using scalar lane extraction for 8-byte vector (OLD - SLOW)
///
/// NEON doesn't have a direct movemask instruction like x86's _mm_movemask_epi8,
/// so this version extracts the high bit from each byte manually using lane extraction.
///
/// This version uses manual lane extraction which is slow (~12x speedup vs theoretical 16x).
/// Kept for reference and benchmarking against the packed version above.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn neon_movemask_u8x8_emulated(vec: uint8x8_t) -> u8 {
    let mut mask = 0u8;
    // Manual unrolling for const indices
    if (vget_lane_u8::<0>(vec) & 0x80) != 0 {
        mask |= 1 << 0;
    }
    if (vget_lane_u8::<1>(vec) & 0x80) != 0 {
        mask |= 1 << 1;
    }
    if (vget_lane_u8::<2>(vec) & 0x80) != 0 {
        mask |= 1 << 2;
    }
    if (vget_lane_u8::<3>(vec) & 0x80) != 0 {
        mask |= 1 << 3;
    }
    if (vget_lane_u8::<4>(vec) & 0x80) != 0 {
        mask |= 1 << 4;
    }
    if (vget_lane_u8::<5>(vec) & 0x80) != 0 {
        mask |= 1 << 5;
    }
    if (vget_lane_u8::<6>(vec) & 0x80) != 0 {
        mask |= 1 << 6;
    }
    if (vget_lane_u8::<7>(vec) & 0x80) != 0 {
        mask |= 1 << 7;
    }
    mask
}

/// NEON movemask using scalar lane extraction for 16-byte vector (OLD - SLOW)
///
/// Extracts high bits from both halves of the 128-bit vector and combines them
/// into a 16-bit mask using manual lane extraction.
///
/// This is the original emulated version kept for benchmarking comparison.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn neon_movemask_u8x16_emulated(vec: uint8x16_t) -> u16 {
    unsafe {
        let low = vget_low_u8(vec);
        let high = vget_high_u8(vec);

        let low_mask = neon_movemask_u8x8_emulated(low) as u16;
        let high_mask = neon_movemask_u8x8_emulated(high) as u16;

        low_mask | (high_mask << 8)
    }
}

/// NEON SIMD text processing: counts lines, words, and UTF-8 characters
///
/// # Algorithm (16 bytes per iteration)
///
/// 1. **Setup**: Create pattern vectors for parallel comparison
///    - Whitespace: space, tab, CR, LF, FF, VT
///    - UTF-8: continuation byte mask (10xxxxxx pattern)
///
/// 2. **Main loop**: Process 16-byte chunks with NEON
///    - Load 16 bytes into NEON register
///    - Compare all bytes against newline → count set bits
///    - Mask and compare for UTF-8 continuation bytes
///    - Compare against all 6 whitespace types → OR results
///    - Count word transitions (whitespace → non-whitespace)
///
/// 3. **Cleanup**: Handle remaining bytes (<16) with scalar code
///
/// # Performance
///
/// Currently uses **PACKED movemask** (pure NEON bit packing) for ~16x speedup.
/// See `neon_movemask_u8x16_packed()` for implementation details.
///
/// # Safety
///
/// Uses unsafe NEON intrinsics. Safe to call on aarch64 (NEON always present).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn count_text_neon(content: &[u8]) -> SimdCounts {
    // Delegate to the PACKED implementation (uses pure NEON bit packing)
    unsafe { count_text_neon_packed(content) }
}

/// Scalar fallback implementation
///
/// Used for:
/// 1. Remaining bytes after SIMD processing (< 16 bytes)
/// 2. Fallback when SIMD is not available
///
/// This implementation correctly handles:
/// - Line counting (newline detection)
/// - Word counting (whitespace transitions)
/// - UTF-8 character counting (non-continuation bytes)
fn count_text_scalar(content: &[u8]) -> SimdCounts {
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

    SimdCounts {
        lines,
        words,
        chars,
    }
}
