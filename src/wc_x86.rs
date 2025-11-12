// //! x86/x86_64 SIMD optimizations for text processing
// //!
// //! This module contains platform-specific optimizations using:
// //! - AVX2: 32 bytes/instruction (Intel Haswell+, AMD Excavator+)
// //! - SSE2: 16 bytes/instruction (almost all x86_64 CPUs)

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::arch::x86_64::*;

use crate::wc_default;
use crate::FileCounts;

/// Internal SIMD results
#[derive(Debug, Clone, Copy)]
struct SimdCounts {
    lines: usize,
    words: usize,
    chars: usize,
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn count_text_simd(content: &[u8]) -> Option<FileCounts> {
    if is_x86_feature_detected!("sse2") {
        let simd_result = unsafe { count_text_sse2(content) };
        return Some(FileCounts {
            lines: simd_result.lines,
            words: simd_result.words,
            bytes: content.len(),
            chars: simd_result.chars,
        });
    }
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
fn count_text_sse2(content: &[u8]) -> SimdCounts {
    const CHUNK_SIZE: usize = 16; // SSE2 processes 16 bytes at a time
    let mut res = SimdCounts {
        lines: 0,
        words: 0,
        chars: 0,
    };
    let newline_vec = _mm_set1_epi8('\n' as i8); // \n  in each lane
    let space_vec = _mm_set1_epi8(b' ' as i8);
    let tab_vec = _mm_set1_epi8(b'\t' as i8);
    let cr_vec = _mm_set1_epi8(b'\r' as i8);
    let ff_vec = _mm_set1_epi8(0x0C as i8);
    let vt_vec = _mm_set1_epi8(0x0B as i8);
    let utf8_cont_mask = _mm_set1_epi8(0b11000000u8 as i8); // Mask to isolate top 2 bits
    let mut chunks = content.chunks_exact(CHUNK_SIZE);

    for chunk in chunks.by_ref() {
        let chunk = unsafe { _mm_loadu_si128(chunk.as_ptr() as *const __m128i) };
        let newlines_mask = unsafe { sse2_extract_newline_mask(chunk, newline_vec) };
        res.lines += newlines_mask.count_ones() as usize;
        let cont_mask = unsafe { sse2_extract_cont_mask(chunk, utf8_cont_mask) };
        res.chars += cont_mask.count_zeros() as usize; //1 is continuation byte so 0 is start of char or whole char in utf-8
    }
    let buf = chunks.remainder();
    if !buf.is_empty() {
        let buf_count = wc_default::word_count_scalar(buf);
        res.chars += buf_count.chars;
        res.lines += buf_count.lines;
        res.words += buf_count.words;
    };
    res
}

/// function extract newline mask -
/// for example a b \n \n c -> 0 0 1 1 0 ...
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn sse2_extract_newline_mask(chunk: __m128i, newline_vec: __m128i) -> u16 {
    _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, newline_vec)) as u16
}

/// function extract continuation mask -
/// For UTF-8 character counting: detect continuation bytes (10xxxxxx)
/// UTF-8 continuation bytes are 0x80-0xBF (binary: 10000000 to 10111111
/// so a->0
///
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn sse2_extract_cont_mask(chunk: __m128i, cont_vec: __m128i) -> u16 {
    let masked_chunk = _mm_and_si128(chunk, cont_vec);
    _mm_movemask_epi8(_mm_cmpeq_epi8(masked_chunk, cont_vec)) as u16
}
