//! x86/x86_64 SIMD optimizations for text processing
//!
//! This module contains platform-specific optimizations using:
//! - AVX2: 32 bytes/instruction (Intel Haswell+, AMD Excavator+)
//! - SSE2: 16 bytes/instruction (almost all x86_64 CPUs)

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
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
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn count_text_simd(content: &[u8]) -> Option<FileCounts> {
    if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512bw") {
        let simd_result = unsafe { count_text_avx512(content) };
        return Some(FileCounts {
            lines: simd_result.lines,
            words: simd_result.words,
            bytes: content.len(),
            chars: simd_result.chars,
        });
    } else if is_x86_feature_detected!("avx2") {
        let simd_result = unsafe { count_text_avx2(content) };
        return Some(FileCounts {
            lines: simd_result.lines,
            words: simd_result.words,
            bytes: content.len(),
            chars: simd_result.chars,
        });
    } else if is_x86_feature_detected!("sse2") {
        let simd_result = unsafe { count_text_sse2(content) };
        return Some(FileCounts {
            lines: simd_result.lines,
            words: simd_result.words,
            bytes: content.len(),
            chars: simd_result.chars,
        });
    }

    // No SIMD available
    None
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn count_text_simd(content: &[u8]) -> Option<FileCounts> {
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
    whitespace_mask: impl Into<u64>,
    chunk_size: usize,
    prev_was_whitespace: &mut bool,
) -> usize {
    let mask = whitespace_mask.into();
    let mut words = 0;

    for bit_idx in 0..chunk_size {
        let is_whitespace = (mask & (1u64 << bit_idx)) != 0;
        if *prev_was_whitespace && !is_whitespace {
            words += 1;
        }
        *prev_was_whitespace = is_whitespace;
    }

    words
}

/// Macro to generate SIMD counting functions
///
/// This macro generates complete SIMD text processing functions by taking:
/// - Different intrinsic function names for each SIMD instruction set
/// - Different vector types and chunk sizes
/// - Helper functions that handle the intrinsic-specific details
///
/// The generated function follows this pattern:
/// 1. Set up SIMD pattern vectors for all whitespace characters and UTF-8 detection
/// 2. Process data in chunks using SIMD operations
/// 3. Extract masks/results using instruction-set-specific helpers
/// 4. Count transitions for word boundaries
/// 5. Handle remaining bytes with scalar processing
macro_rules! simd_count_function {
    (
        $fn_name:ident,                    // Name of the generated function (e.g., count_text_avx512_new)
        $target_features:literal,          // Target features for #[target_feature] (e.g., "avx512f,avx512bw")
        $chunk_size:expr,                  // Bytes processed per iteration (64 for AVX-512, 32 for AVX2, 16 for SSE2)
        $vector_type:ty,                   // SIMD vector type (__m512i, __m256i, __m128i)
        $set1_epi8:ident,                 // Intrinsic to create pattern vector (_mm512_set1_epi8, _mm256_set1_epi8, _mm_set1_epi8)
        $loadu:ident,                     // Intrinsic to load unaligned data (_mm512_loadu_si512, _mm256_loadu_si256, _mm_loadu_si128)
        $and:ident,                       // Intrinsic for bitwise AND (_mm512_and_si512, _mm256_and_si256, _mm_and_si128)
        $cmpeq_epi8:ident,               // Intrinsic for byte comparison (_mm512_cmpeq_epi8, _mm256_cmpeq_epi8, _mm_cmpeq_epi8)
        $extract_newline_mask:expr,       // Helper function to extract newline comparison results as mask
        $extract_continuation_mask:expr,  // Helper function to extract UTF-8 continuation byte mask
        $extract_whitespace_masks:expr    // Helper function to combine all whitespace character masks
    ) => {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        #[target_feature(enable = $target_features)]
        unsafe fn $fn_name(content: &[u8]) -> SimdCounts {
            let mut lines = 0;
            let mut words = 0;
            let mut chars = 0;

            // Create SIMD pattern vectors - each vector contains the same byte repeated across all lanes
            // These will be used to compare against loaded data chunks
            let newline_vec = $set1_epi8(b'\n' as i8); // Vector of newline characters
            let space_vec = $set1_epi8(b' ' as i8); // Vector of space characters
            let tab_vec = $set1_epi8(b'\t' as i8); // Vector of tab characters
            let cr_vec = $set1_epi8(b'\r' as i8); // Vector of carriage return characters
            let ff_vec = $set1_epi8(0x0C as i8); // Vector of form feed characters
            let vt_vec = $set1_epi8(0x0B as i8); // Vector of vertical tab characters

            // UTF-8 character counting setup
            // UTF-8 continuation bytes have the bit pattern 10xxxxxx (0x80-0xBF)
            // We mask the top 2 bits and compare against the continuation pattern
            let utf8_cont_mask = $set1_epi8(0b11000000u8 as i8); // Mask to isolate top 2 bits
            let utf8_cont_pattern = $set1_epi8(0b10000000u8 as i8); // Pattern for continuation bytes

            // Calculate how many complete chunks we can process with SIMD
            let chunks = content.len() / $chunk_size;
            let mut i = 0; // Current byte position in the input
            let mut prev_was_whitespace = true; // Track word boundaries across chunks

            // SIMD processing loop - process $chunk_size bytes at a time
            for _ in 0..chunks {
                unsafe {
                    // Load $chunk_size bytes from memory into a SIMD register
                    // Uses unaligned load since text data may not be aligned to SIMD boundaries
                    let chunk = $loadu(content.as_ptr().add(i) as *const $vector_type);

                    // Count newlines by comparing each byte in the chunk against '\n'
                    // Helper function handles instruction-set-specific mask extraction
                    let newline_mask = $extract_newline_mask(chunk, newline_vec);
                    lines += newline_mask.count_ones() as usize;

                    // Count UTF-8 characters by detecting non-continuation bytes
                    // 1. Mask each byte to get only the top 2 bits
                    let masked_chunk = $and(chunk, utf8_cont_mask);
                    // 2. Compare against continuation pattern (10xxxxxx)
                    let continuation_mask =
                        $extract_continuation_mask(masked_chunk, utf8_cont_pattern);
                    // 3. Count non-continuation bytes (each UTF-8 char starts with a non-continuation byte)
                    chars += $chunk_size - continuation_mask.count_ones() as usize;

                    // Detect all ASCII whitespace characters and combine their masks
                    // Helper function handles the instruction-set-specific combining logic
                    let whitespace_mask = $extract_whitespace_masks(
                        chunk,
                        space_vec,
                        tab_vec,
                        cr_vec,
                        newline_vec,
                        ff_vec,
                        vt_vec,
                    );

                    // Count word transitions (whitespace to non-whitespace boundaries)
                    // This updates prev_was_whitespace for the next iteration
                    words += count_word_transitions(
                        whitespace_mask,
                        $chunk_size,
                        &mut prev_was_whitespace,
                    );
                }

                // Move to the next chunk - advance by exactly $chunk_size bytes
                i += $chunk_size;
            }

            // Handle any remaining bytes that don't fill a complete SIMD chunk
            // Uses scalar processing and handles word boundary edge cases
            handle_scalar_boundary(content, i, prev_was_whitespace, words, lines, chars)
        }
    };
}

// Helper functions for AVX-512

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn avx512_extract_newline_mask(chunk: __m512i, newline_vec: __m512i) -> u64 {
    _mm512_cmpeq_epi8_mask(chunk, newline_vec)
}

/// Extract UTF-8 continuation byte mask using AVX-512 instructions
///
/// This function detects UTF-8 continuation bytes in a 64-byte chunk to enable accurate
/// character counting in UTF-8 text. UTF-8 is a variable-length encoding where:
///
/// - Single-byte characters (ASCII): 0xxxxxxx (0x00-0x7F)
/// - Multi-byte character start bytes: 110xxxxx, 1110xxxx, 11110xxx (0xC0-0xF7)
/// - Continuation bytes: 10xxxxxx (0x80-0xBF)
///
/// To count UTF-8 characters correctly, we count all bytes that are NOT continuation bytes,
/// since each UTF-8 character starts with exactly one non-continuation byte.
///
/// # Parameters
/// - `masked_chunk`: 64 bytes where each byte has been masked with 0b11000000 (top 2 bits only)
/// - `utf8_cont_pattern`: Vector filled with 0b10000000 (the continuation byte pattern)
///
/// # Returns
/// A 64-bit mask where each bit indicates if the corresponding byte is a UTF-8 continuation byte:
/// - Bit = 1: This byte is a continuation byte (10xxxxxx pattern)
/// - Bit = 0: This byte is NOT a continuation byte (start of a UTF-8 character)
///
/// # Example
/// For the UTF-8 string "Hé" (ASCII 'H' + 2-byte 'é'):
/// - Input bytes: [0x48, 0xC3, 0xA9] = ['H', start_of_é, continuation_of_é]
/// - After masking: [0x00, 0xC0, 0x80] (only top 2 bits kept)
/// - Comparison result: [false, false, true] (only 0xA9 matches 10xxxxxx pattern)
/// - Returned mask: 0b100 (bit 2 set, indicating byte 2 is continuation)
/// - Character count: 3 - 1 = 2 characters (correct for "Hé")
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn avx512_extract_continuation_mask(
    masked_chunk: __m512i,
    utf8_cont_pattern: __m512i,
) -> u64 {
    _mm512_cmpeq_epi8_mask(masked_chunk, utf8_cont_pattern)
}

/// Extract combined whitespace mask using AVX-512 instructions
///
/// This function detects all ASCII whitespace characters in a 64-byte chunk by comparing
/// against all 6 whitespace types and combining the results into a single mask.
///
/// # ASCII Whitespace Characters Detected
/// - Space (0x20): Most common whitespace
/// - Tab (0x09): Horizontal tab character
/// - Newline (0x0A): Line feed character
/// - Carriage Return (0x0D): Often paired with newline in Windows
/// - Form Feed (0x0C): Page break character
/// - Vertical Tab (0x0B): Vertical spacing character
///
/// # How it works
/// 1. **Parallel Comparisons**: Uses `_mm512_cmpeq_epi8_mask` to compare all 64 bytes
///    simultaneously against each whitespace pattern vector
/// 2. **Direct Mask Results**: AVX-512 mask instructions return bit masks directly
///    without needing movemask operations
/// 3. **Bitwise Combination**: Uses OR operations to combine all 6 comparison masks
///    into a single unified whitespace mask
///
/// # AVX-512 advantages
/// - Processes 64 bytes per call (vs 32 for AVX2, 16 for SSE2)
/// - Direct mask operations are faster than vector+movemask approach
/// - Single instruction produces bit mask for each comparison
///
/// # Parameters
/// - `chunk`: 64 bytes of text data in AVX-512 register
/// - `space_vec`, `tab_vec`, etc.: Pattern vectors for each whitespace type
///
/// # Returns
/// A 64-bit mask where each bit indicates if the corresponding byte is any whitespace:
/// - Bit = 1: This byte is a whitespace character
/// - Bit = 0: This byte is not whitespace
///
/// # Example
/// For input "Hi\tworld\n":
/// - Input bytes: ['H','i','\t','w','o','r','l','d','\n', ...]
/// - Individual masks: space=0, tab=0b100, cr=0, newline=0b1000000000, ff=0, vt=0
/// - Combined mask: 0b1000000100 (bits 2 and 8 set for tab and newline)
/// - Word counting can use this mask to detect word boundaries
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn avx512_extract_whitespace_masks(
    chunk: __m512i,
    space_vec: __m512i,
    tab_vec: __m512i,
    cr_vec: __m512i,
    newline_vec: __m512i,
    ff_vec: __m512i,
    vt_vec: __m512i,
) -> u64 {
    let space_mask = _mm512_cmpeq_epi8_mask(chunk, space_vec);
    let tab_mask = _mm512_cmpeq_epi8_mask(chunk, tab_vec);
    let cr_mask = _mm512_cmpeq_epi8_mask(chunk, cr_vec);
    let newline_mask = _mm512_cmpeq_epi8_mask(chunk, newline_vec);
    let ff_mask = _mm512_cmpeq_epi8_mask(chunk, ff_vec);
    let vt_mask = _mm512_cmpeq_epi8_mask(chunk, vt_vec);

    space_mask | tab_mask | cr_mask | newline_mask | ff_mask | vt_mask
}

// Helper functions for AVX2

/// Extract newline byte mask using AVX2 instructions
///
/// This function detects newline characters ('\n', ASCII 0x0A) in a 32-byte chunk
/// for efficient line counting in text processing applications.
///
/// # How it works
/// 1. **Parallel Comparison**: Uses `_mm256_cmpeq_epi8` to compare all 32 bytes
///    simultaneously against the newline pattern vector
/// 2. **Result Vector**: Each byte position becomes 0xFF if it matches '\n',
///    or 0x00 if it doesn't match
/// 3. **Mask Extraction**: Uses `_mm256_movemask_epi8` to extract the high bit
///    of each byte into a compact 32-bit mask
///
/// # AVX2-specific details
/// - Processes 32 bytes per call (vs 64 for AVX-512, 16 for SSE2)
/// - Uses vector comparison that returns a vector (not direct mask like AVX-512)
/// - Requires mask extraction step to convert vector result to bit mask
/// - Available on Intel Haswell+ (2013) and AMD Excavator+ (2015) CPUs
///
/// # Parameters
/// - `chunk`: 32 bytes of text data loaded into AVX2 register
/// - `newline_vec`: Vector filled with '\n' characters for parallel comparison
///
/// # Returns
/// A 32-bit mask where each bit indicates if the corresponding byte is a newline:
/// - Bit = 1: This byte is '\n' (newline character)
/// - Bit = 0: This byte is not '\n'
///
/// # Example
/// For input bytes "Hello\nWorld\nText":
/// - Input: ['H','e','l','l','o','\n','W','o','r','l','d','\n','T','e','x','t', ...]
/// - Comparison results: [0x00,0x00,0x00,0x00,0x00,0xFF,0x00,0x00,0x00,0x00,0x00,0xFF,0x00,0x00,0x00,0x00, ...]
/// - Extracted mask: 0b000000100000100000000000 = bits 5 and 11 set
/// - Line count: 2 newlines found (mask.count_ones() = 2)
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn avx2_extract_newline_mask(chunk: __m256i, newline_vec: __m256i) -> u32 {
    _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, newline_vec)) as u32
}

/// Extract UTF-8 continuation byte mask using AVX2 instructions
///
/// Similar to the AVX-512 version but processes 32 bytes at a time and uses
/// different intrinsics for mask extraction.
///
/// # AVX2-specific details
/// - Processes 32 bytes per call (vs 64 for AVX-512, 16 for SSE2)
/// - Uses `_mm256_cmpeq_epi8` for comparison (returns vector, not mask)
/// - Uses `_mm256_movemask_epi8` to extract high bits into a 32-bit mask
///
/// # Returns
/// A 32-bit mask where each bit indicates UTF-8 continuation bytes
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn avx2_extract_continuation_mask(masked_chunk: __m256i, utf8_cont_pattern: __m256i) -> u32 {
    _mm256_movemask_epi8(_mm256_cmpeq_epi8(masked_chunk, utf8_cont_pattern)) as u32
}

/// Extract combined whitespace mask using AVX2 instructions
///
/// This function detects all ASCII whitespace characters in a 32-byte chunk by comparing
/// against all 6 whitespace types and combining the results using vector operations.
///
/// # ASCII Whitespace Characters Detected
/// - Space (0x20): Most common whitespace
/// - Tab (0x09): Horizontal tab character
/// - Newline (0x0A): Line feed character
/// - Carriage Return (0x0D): Often paired with newline in Windows
/// - Form Feed (0x0C): Page break character
/// - Vertical Tab (0x0B): Vertical spacing character
///
/// # How it works
/// 1. **Parallel Comparisons**: Uses `_mm256_cmpeq_epi8` to compare all 32 bytes
///    simultaneously against each whitespace pattern vector
/// 2. **Vector Combination**: Uses `_mm256_or_si256` to combine comparison vectors
///    through a tree of OR operations for efficiency
/// 3. **Mask Extraction**: Uses `_mm256_movemask_epi8` to extract the final
///    combined result as a 32-bit mask
///
/// # AVX2-specific approach
/// - Processes 32 bytes per call (vs 64 for AVX-512, 16 for SSE2)
/// - Uses vector operations requiring explicit combination and mask extraction
/// - Tree-structured OR operations minimize instruction count
/// - Compatible with Intel Haswell+ (2013) and AMD Excavator+ (2015)
///
/// # Parameters
/// - `chunk`: 32 bytes of text data in AVX2 register
/// - `space_vec`, `tab_vec`, etc.: Pattern vectors for each whitespace type
///
/// # Returns
/// A 32-bit mask where each bit indicates if the corresponding byte is any whitespace:
/// - Bit = 1: This byte is a whitespace character
/// - Bit = 0: This byte is not whitespace
///
/// # Example
/// For input "Hello\r\nworld\t":
/// - Input bytes: ['H','e','l','l','o','\r','\n','w','o','r','l','d','\t', ...]
/// - Individual comparisons create vectors with 0xFF for matches, 0x00 for non-matches
/// - OR operations combine: space|tab, cr|newline, ff|vt, then combine all
/// - Final mask: 0b1000000001100000 (bits 5,6,12 set for \r,\n,\t)
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn avx2_extract_whitespace_masks(
    chunk: __m256i,
    space_vec: __m256i,
    tab_vec: __m256i,
    cr_vec: __m256i,
    newline_vec: __m256i,
    ff_vec: __m256i,
    vt_vec: __m256i,
) -> u32 {
    let space_cmp = _mm256_cmpeq_epi8(chunk, space_vec);
    let tab_cmp = _mm256_cmpeq_epi8(chunk, tab_vec);
    let cr_cmp = _mm256_cmpeq_epi8(chunk, cr_vec);
    let newline_cmp = _mm256_cmpeq_epi8(chunk, newline_vec);
    let ff_cmp = _mm256_cmpeq_epi8(chunk, ff_vec);
    let vt_cmp = _mm256_cmpeq_epi8(chunk, vt_vec);

    let ws1 = _mm256_or_si256(space_cmp, tab_cmp);
    let ws2 = _mm256_or_si256(cr_cmp, newline_cmp);
    let ws3 = _mm256_or_si256(ff_cmp, vt_cmp);
    let ws_combined = _mm256_or_si256(ws1, ws2);

    _mm256_movemask_epi8(_mm256_or_si256(ws_combined, ws3)) as u32
}

// Helper functions for SSE2

/// Extract newline byte mask using SSE2 instructions
///
/// This function detects newline characters ('\n', ASCII 0x0A) in a 16-byte chunk
/// using the most widely compatible x86_64 SIMD instruction set.
///
/// # How it works
/// 1. **Parallel Comparison**: Uses `_mm_cmpeq_epi8` to compare all 16 bytes
///    simultaneously against the newline pattern vector
/// 2. **Result Vector**: Each byte position becomes 0xFF if it matches '\n',
///    or 0x00 if it doesn't match
/// 3. **Mask Extraction**: Uses `_mm_movemask_epi8` to extract the high bit
///    of each byte into a compact 16-bit mask
///
/// # SSE2-specific details
/// - Processes 16 bytes per call (vs 32 for AVX2, 64 for AVX-512)
/// - Uses 128-bit vector operations (baseline for all x86_64 CPUs)
/// - Same vector+movemask approach as AVX2 but with smaller chunks
/// - Maximum compatibility - available on virtually all x86_64 systems
/// - Part of the original x86_64 specification since AMD64 introduction
///
/// # Parameters
/// - `chunk`: 16 bytes of text data loaded into SSE2 register
/// - `newline_vec`: Vector filled with '\n' characters for parallel comparison
///
/// # Returns
/// A 16-bit mask where each bit indicates if the corresponding byte is a newline:
/// - Bit = 1: This byte is '\n' (newline character)
/// - Bit = 0: This byte is not '\n'
///
/// # Example
/// For input bytes "Short\ntext\n":
/// - Input: ['S','h','o','r','t','\n','t','e','x','t','\n', padding...]
/// - Comparison results: [0x00,0x00,0x00,0x00,0x00,0xFF,0x00,0x00,0x00,0x00,0xFF,0x00,0x00,0x00,0x00,0x00]
/// - Extracted mask: 0b0000101000100000 = bits 5 and 10 set
/// - Line count: 2 newlines found (mask.count_ones() = 2)
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn sse2_extract_newline_mask(chunk: __m128i, newline_vec: __m128i) -> u16 {
    _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, newline_vec)) as u16
}

/// Extract UTF-8 continuation byte mask using SSE2 instructions
///
/// The most compatible version that works on virtually all x86_64 CPUs.
/// Uses the same UTF-8 detection logic but processes only 16 bytes at a time.
///
/// # SSE2-specific details
/// - Processes 16 bytes per call (smallest chunk size)
/// - Uses `_mm_cmpeq_epi8` for comparison (128-bit vector operations)
/// - Uses `_mm_movemask_epi8` to extract high bits into a 16-bit mask
/// - Available on all x86_64 CPUs (part of the baseline instruction set)
///
/// # Returns
/// A 16-bit mask where each bit indicates UTF-8 continuation bytes
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn sse2_extract_continuation_mask(masked_chunk: __m128i, utf8_cont_pattern: __m128i) -> u16 {
    _mm_movemask_epi8(_mm_cmpeq_epi8(masked_chunk, utf8_cont_pattern)) as u16
}

/// Extract combined whitespace mask using SSE2 instructions
///
/// This function detects all ASCII whitespace characters in a 16-byte chunk using
/// the most compatible x86_64 SIMD instruction set available on all systems.
///
/// # ASCII Whitespace Characters Detected
/// - Space (0x20): Most common whitespace
/// - Tab (0x09): Horizontal tab character
/// - Newline (0x0A): Line feed character
/// - Carriage Return (0x0D): Often paired with newline in Windows
/// - Form Feed (0x0C): Page break character
/// - Vertical Tab (0x0B): Vertical spacing character
///
/// # How it works
/// 1. **Parallel Comparisons**: Uses `_mm_cmpeq_epi8` to compare all 16 bytes
///    simultaneously against each whitespace pattern vector
/// 2. **Vector Combination**: Uses `_mm_or_si128` to combine comparison vectors
///    through a tree of OR operations for efficiency
/// 3. **Mask Extraction**: Uses `_mm_movemask_epi8` to extract the final
///    combined result as a 16-bit mask
///
/// # SSE2-specific approach
/// - Processes 16 bytes per call (smallest SIMD chunk size)
/// - Uses 128-bit vector operations with explicit combination steps
/// - Tree-structured OR operations minimize instruction count
/// - Maximum compatibility - works on all x86_64 CPUs since 2003
/// - Fallback option when newer instruction sets aren't available
///
/// # Parameters
/// - `chunk`: 16 bytes of text data in SSE2 register
/// - `space_vec`, `tab_vec`, etc.: Pattern vectors for each whitespace type
///
/// # Returns
/// A 16-bit mask where each bit indicates if the corresponding byte is any whitespace:
/// - Bit = 1: This byte is a whitespace character
/// - Bit = 0: This byte is not whitespace
///
/// # Example
/// For input "Text\n\tmore":
/// - Input bytes: ['T','e','x','t','\n','\t','m','o','r','e', padding...]
/// - Individual comparisons create vectors with 0xFF for matches, 0x00 for non-matches
/// - OR operations combine: space|tab, cr|newline, ff|vt, then combine all
/// - Final mask: 0b0000000001100000 (bits 4,5 set for \n,\t)
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn sse2_extract_whitespace_masks(
    chunk: __m128i,
    space_vec: __m128i,
    tab_vec: __m128i,
    cr_vec: __m128i,
    newline_vec: __m128i,
    ff_vec: __m128i,
    vt_vec: __m128i,
) -> u16 {
    let space_cmp = _mm_cmpeq_epi8(chunk, space_vec);
    let tab_cmp = _mm_cmpeq_epi8(chunk, tab_vec);
    let cr_cmp = _mm_cmpeq_epi8(chunk, cr_vec);
    let newline_cmp = _mm_cmpeq_epi8(chunk, newline_vec);
    let ff_cmp = _mm_cmpeq_epi8(chunk, ff_vec);
    let vt_cmp = _mm_cmpeq_epi8(chunk, vt_vec);

    let ws1 = _mm_or_si128(space_cmp, tab_cmp);
    let ws2 = _mm_or_si128(cr_cmp, newline_cmp);
    let ws3 = _mm_or_si128(ff_cmp, vt_cmp);
    let ws_combined = _mm_or_si128(ws1, ws2);

    _mm_movemask_epi8(_mm_or_si128(ws_combined, ws3)) as u16
}

// Generate all three SIMD functions using the macro

// AVX-512 implementation: processes 64 bytes per iteration
// Uses mask-based operations which return bit masks directly
simd_count_function!(
    count_text_avx512,                // Function name
    "avx512f,avx512bw",               // Required CPU features
    64,                               // Chunk size (bytes per iteration)
    __m512i,                          // SIMD vector type (512-bit)
    _mm512_set1_epi8,                 // Create pattern vector intrinsic
    _mm512_loadu_si512,               // Load unaligned data intrinsic
    _mm512_and_si512,                 // Bitwise AND intrinsic
    _mm512_cmpeq_epi8,                // Byte comparison intrinsic
    avx512_extract_newline_mask,      // Helper: extract newline comparison mask
    avx512_extract_continuation_mask, // Helper: extract UTF-8 continuation mask
    avx512_extract_whitespace_masks   // Helper: combine all whitespace masks
);

// AVX2 implementation: processes 32 bytes per iteration
// Uses vector operations that require movemask to extract results
simd_count_function!(
    count_text_avx2,                // Function name
    "avx2",                         // Required CPU features
    32,                             // Chunk size (bytes per iteration)
    __m256i,                        // SIMD vector type (256-bit)
    _mm256_set1_epi8,               // Create pattern vector intrinsic
    _mm256_loadu_si256,             // Load unaligned data intrinsic
    _mm256_and_si256,               // Bitwise AND intrinsic
    _mm256_cmpeq_epi8,              // Byte comparison intrinsic
    avx2_extract_newline_mask,      // Helper: extract newline comparison mask
    avx2_extract_continuation_mask, // Helper: extract UTF-8 continuation mask
    avx2_extract_whitespace_masks   // Helper: combine all whitespace masks
);

// SSE2 implementation: processes 16 bytes per iteration
// Most compatible x86_64 SIMD instruction set
simd_count_function!(
    count_text_sse2,                // Function name
    "sse2",                         // Required CPU features
    16,                             // Chunk size (bytes per iteration)
    __m128i,                        // SIMD vector type (128-bit)
    _mm_set1_epi8,                  // Create pattern vector intrinsic
    _mm_loadu_si128,                // Load unaligned data intrinsic
    _mm_and_si128,                  // Bitwise AND intrinsic
    _mm_cmpeq_epi8,                 // Byte comparison intrinsic
    sse2_extract_newline_mask,      // Helper: extract newline comparison mask
    sse2_extract_continuation_mask, // Helper: extract UTF-8 continuation mask
    sse2_extract_whitespace_masks   // Helper: combine all whitespace masks
);

// ===================================================================
// MANUAL IMPLEMENTATIONS (KEPT FOR DOCUMENTATION PURPOSES)
//
// The functions below are the original hand-written implementations
// that the macro system replaced. They are kept here to show what
// the macro expands to and for comparison purposes.
//
// See MACRO_EXPANSION_DOCS.md for detailed macro expansion examples.
// ===================================================================

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx512f,avx512bw")]
#[allow(dead_code)]
unsafe fn count_text_avx512_manual(content: &[u8]) -> SimdCounts {
    let mut lines = 0;
    let mut words = 0;
    let mut chars = 0;

    // SIMD vectors for comparison
    let newline_vec = _mm512_set1_epi8(b'\n' as i8);
    let space_vec = _mm512_set1_epi8(b' ' as i8);
    let tab_vec = _mm512_set1_epi8(b'\t' as i8);
    let cr_vec = _mm512_set1_epi8(b'\r' as i8);
    let ff_vec = _mm512_set1_epi8(0x0C as i8);
    let vt_vec = _mm512_set1_epi8(0x0B as i8);

    // For UTF-8 character counting
    let utf8_cont_mask = _mm512_set1_epi8(0b11000000u8 as i8);
    let utf8_cont_pattern = _mm512_set1_epi8(0b10000000u8 as i8);

    let chunks = content.len() / 64;
    let mut i = 0;
    let mut prev_was_whitespace = true;

    // Process 64-byte chunks with AVX-512
    for _ in 0..chunks {
        unsafe {
            let chunk = _mm512_loadu_si512(content.as_ptr().add(i) as *const __m512i);

            // Count newlines
            let newline_mask = _mm512_cmpeq_epi8_mask(chunk, newline_vec);
            lines += newline_mask.count_ones() as usize;

            // Count UTF-8 characters
            let masked_chunk = _mm512_and_si512(chunk, utf8_cont_mask);
            let continuation_mask = _mm512_cmpeq_epi8_mask(masked_chunk, utf8_cont_pattern);
            chars += 64 - continuation_mask.count_ones() as usize;

            // Detect whitespace
            let space_mask = _mm512_cmpeq_epi8_mask(chunk, space_vec);
            let tab_mask = _mm512_cmpeq_epi8_mask(chunk, tab_vec);
            let cr_mask = _mm512_cmpeq_epi8_mask(chunk, cr_vec);
            let newline_mask_for_words = _mm512_cmpeq_epi8_mask(chunk, newline_vec);
            let ff_mask = _mm512_cmpeq_epi8_mask(chunk, ff_vec);
            let vt_mask = _mm512_cmpeq_epi8_mask(chunk, vt_vec);

            let whitespace_mask =
                space_mask | tab_mask | cr_mask | newline_mask_for_words | ff_mask | vt_mask;

            // Count word transitions using helper function
            words += count_word_transitions(whitespace_mask, 64, &mut prev_was_whitespace);
        }
        i += 64;
    }

    // Handle remaining bytes using helper function
    handle_scalar_boundary(content, i, prev_was_whitespace, words, lines, chars)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
#[allow(dead_code)]
unsafe fn count_text_avx2_manual(content: &[u8]) -> SimdCounts {
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
    let ff_vec = _mm256_set1_epi8(0x0C as i8); // ['\f', '\f', '\f', ... 32 times] (form feed)
    let vt_vec = _mm256_set1_epi8(0x0B as i8); // ['\v', '\v', '\v', ... 32 times] (vertical tab)

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

            // Detect whitespace for word counting (all 6 ASCII whitespace characters)
            // We check for space, tab, carriage return, newline, form feed, and vertical tab simultaneously
            let space_cmp = _mm256_cmpeq_epi8(chunk, space_vec);
            let tab_cmp = _mm256_cmpeq_epi8(chunk, tab_vec);
            let cr_cmp = _mm256_cmpeq_epi8(chunk, cr_vec);
            let newline_cmp_for_words = _mm256_cmpeq_epi8(chunk, newline_vec);
            let ff_cmp = _mm256_cmpeq_epi8(chunk, ff_vec);
            let vt_cmp = _mm256_cmpeq_epi8(chunk, vt_vec);

            // Combine all whitespace comparisons using OR operations
            // _mm256_or_si256: Bitwise OR operation on 32 bytes simultaneously
            // This combines multiple comparison results into one
            let ws1 = _mm256_or_si256(space_cmp, tab_cmp); // space OR tab
            let ws2 = _mm256_or_si256(cr_cmp, newline_cmp_for_words); // CR OR newline
            let ws3 = _mm256_or_si256(ff_cmp, vt_cmp); // form feed OR vertical tab
            let ws_combined = _mm256_or_si256(ws1, ws2); // (space|tab) OR (CR|newline)
            let whitespace_mask = _mm256_movemask_epi8(_mm256_or_si256(ws_combined, ws3)) as u32;

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
            words += scalar_result.words.saturating_sub(1);
        } else {
            // The first scalar byte is whitespace, so it ends the SIMD word
            words += scalar_result.words;
        }
    } else {
        // No boundary issues - just add the scalar word count
        words += scalar_result.words;
    }

    // Add the scalar results for lines and characters
    lines += scalar_result.lines; // lines from remaining bytes
    chars += scalar_result.chars; // characters from remaining bytes

    SimdCounts {
        lines,
        words,
        chars,
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
#[allow(dead_code)]
unsafe fn count_text_sse2_manual(content: &[u8]) -> SimdCounts {
    let mut lines = 0;
    let mut words = 0;
    let mut chars = 0;

    // SIMD vectors for comparison
    // SSE2 works by processing 16 bytes in parallel (half of AVX2's 32 bytes)
    // We create "pattern vectors" filled with the same byte value to compare against.

    // _mm_set1_epi8: Creates a 128-bit vector (16 bytes) where every byte is the same value
    // This lets us compare all 16 bytes in a chunk against '\n' simultaneously
    let newline_vec = _mm_set1_epi8(b'\n' as i8); // ['\n', '\n', '\n', ... 16 times]
    let space_vec = _mm_set1_epi8(b' ' as i8); // [' ', ' ', ' ', ... 16 times]
    let tab_vec = _mm_set1_epi8(b'\t' as i8); // ['\t', '\t', '\t', ... 16 times]
    let cr_vec = _mm_set1_epi8(b'\r' as i8); // ['\r', '\r', '\r', ... 16 times]
    let ff_vec = _mm_set1_epi8(0x0C as i8); // ['\f', '\f', '\f', ... 16 times] (form feed)
    let vt_vec = _mm_set1_epi8(0x0B as i8); // ['\v', '\v', '\v', ... 16 times] (vertical tab)

    // For UTF-8 character counting: detect continuation bytes (10xxxxxx)
    // UTF-8 continuation bytes are 0x80-0xBF (binary: 10000000 to 10111111)
    // To detect them, we mask the top 2 bits and check if they equal 10xxxxxx
    let utf8_cont_mask = _mm_set1_epi8(0b11000000u8 as i8); // Mask to isolate top 2 bits
    let utf8_cont_pattern = _mm_set1_epi8(0b10000000u8 as i8); // Pattern 10xxxxxx

    let chunks = content.len() / 16; // How many 16-byte chunks we can process
    let mut i = 0;
    let mut prev_was_whitespace = true; // Assume we start after whitespace for word counting

    // Process 16-byte chunks with SSE2
    for _ in 0..chunks {
        unsafe {
            // Load 16 bytes from memory into a SIMD register
            // _mm_loadu_si128: Loads 128 bits (16 bytes) from unaligned memory
            // "unaligned" means the data doesn't have to be at a special memory address
            let chunk = _mm_loadu_si128(content.as_ptr().add(i) as *const __m128i);

            // Count newlines: Compare each byte in chunk with '\n'
            // _mm_cmpeq_epi8: Compares 16 bytes simultaneously
            // Returns a vector where each byte is 0xFF if equal, 0x00 if not equal
            let newline_cmp = _mm_cmpeq_epi8(chunk, newline_vec);

            // _mm_movemask_epi8: Extracts the high bit of each byte into a 16-bit mask
            // Since 0xFF has high bit = 1 and 0x00 has high bit = 0, this gives us a bitmask
            // where bit N is 1 if byte N was a newline
            let newline_mask = _mm_movemask_epi8(newline_cmp) as u16;

            // count_ones(): Counts how many bits are set in the mask = number of newlines found
            lines += newline_mask.count_ones() as usize;

            // Count UTF-8 characters by counting non-continuation bytes
            // Strategy: UTF-8 continuation bytes have pattern 10xxxxxx
            // We mask each byte with 11000000 to get the top 2 bits, then check if they equal 10000000

            // _mm_and_si128: Bitwise AND operation on 16 bytes simultaneously
            // This masks out the bottom 6 bits, keeping only the top 2 bits
            let masked_chunk = _mm_and_si128(chunk, utf8_cont_mask);

            // Compare the masked bytes with the continuation pattern (10000000)
            let is_continuation = _mm_cmpeq_epi8(masked_chunk, utf8_cont_pattern);
            let continuation_mask = _mm_movemask_epi8(is_continuation) as u16;

            // Count non-continuation bytes = UTF-8 character count
            // Each UTF-8 character starts with a non-continuation byte
            chars += 16 - continuation_mask.count_ones() as usize;
            // Detect whitespace for word counting (all 6 ASCII whitespace characters)
            // We check for space, tab, carriage return, newline, form feed, and vertical tab simultaneously
            let space_cmp = _mm_cmpeq_epi8(chunk, space_vec);
            let tab_cmp = _mm_cmpeq_epi8(chunk, tab_vec);
            let cr_cmp = _mm_cmpeq_epi8(chunk, cr_vec);
            let newline_cmp_for_words = _mm_cmpeq_epi8(chunk, newline_vec);
            let ff_cmp = _mm_cmpeq_epi8(chunk, ff_vec);
            let vt_cmp = _mm_cmpeq_epi8(chunk, vt_vec);

            // Combine all whitespace comparisons using OR operations
            // _mm_or_si128: Bitwise OR operation on 16 bytes simultaneously
            // This combines multiple comparison results into one
            let ws1 = _mm_or_si128(space_cmp, tab_cmp); // space OR tab
            let ws2 = _mm_or_si128(cr_cmp, newline_cmp_for_words); // CR OR newline
            let ws3 = _mm_or_si128(ff_cmp, vt_cmp); // form feed OR vertical tab
            let ws_combined = _mm_or_si128(ws1, ws2); // (space|tab) OR (CR|newline)
            let whitespace_mask = _mm_movemask_epi8(_mm_or_si128(ws_combined, ws3)) as u16;

            // Count word transitions (whitespace to non-whitespace)
            // We iterate through each bit in the mask to track word boundaries
            for bit_idx in 0..16 {
                // Check if this byte position is whitespace
                let is_whitespace = (whitespace_mask & (1 << bit_idx)) != 0;

                // A new word starts when we transition from whitespace to non-whitespace
                if prev_was_whitespace && !is_whitespace {
                    words += 1;
                }
                prev_was_whitespace = is_whitespace;
            }
        }

        i += 16; // Move to the next 16-byte chunk
    }

    // Handle remaining bytes with scalar processing
    // After processing all complete 16-byte chunks, we may have some leftover bytes
    // (e.g., if file is 100 bytes, we process 96 bytes with SIMD, 4 bytes with scalar)
    let scalar_result = count_text_scalar(&content[i..]);

    // Adjust for transition from SIMD to scalar processing
    // We need to be careful not to double-count words that span the SIMD/scalar boundary
    if i > 0 && i < content.len() && !prev_was_whitespace {
        // We ended SIMD processing in the middle of a word
        let first_remaining_byte = content[i];
        if !is_ascii_whitespace(first_remaining_byte) {
            // The first scalar byte continues the word from SIMD, so don't double-count
            words += scalar_result.words.saturating_sub(1);
        } else {
            // The first scalar byte is whitespace, so it ends the SIMD word
            words += scalar_result.words;
        }
    } else {
        // No boundary issues - just add the scalar word count
        words += scalar_result.words;
    }

    // Add the scalar results for lines and characters
    lines += scalar_result.lines; // lines from remaining bytes
    chars += scalar_result.chars; // characters from remaining bytes

    SimdCounts {
        lines,
        words,
        chars,
    }
}

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
