


#[cfg(target_arch = "aarch64")]
use  std::arch::aarch64::*;
use crate::FileCounts;
use crate::wc_default;


/// Attempts SIMD-accelerated text counting on ARM64 processors.
///
/// Returns `Some(FileCounts)` if NEON instructions are available, `None` otherwise.
#[cfg(target_arch = "aarch64")]
pub fn count_text_simd(content: &[u8]) -> Option<FileCounts> {
    if std::arch::is_aarch64_feature_detected!("neon") {
        return  Some(unsafe {
            count_text_neon(content)
        })
    }
    None
}

/// Counts lines, words, and UTF-8 characters using ARM NEON SIMD instructions.
///
/// Processes input in 16-byte chunks. 
/// Falls back to scalar counting for remaining bytes.
unsafe fn count_text_neon(content: &[u8]) -> FileCounts {
    unsafe{
    let mut res=FileCounts{
        lines:0,
        words:0,
        chars:0,
        bytes:content.len()
    };
    const CHUNK_SIZE: usize = 16; // neon lane is 16 lanes(128 bits)
    let mut chunks = content.chunks_exact(CHUNK_SIZE);
    let newline_vec =  vdupq_n_u8(b'\n');

    // Optimized whitespace detection: range [0x09-0x0D] covers tab, LF, VT, FF, CR
    let ws_range_min = vdupq_n_u8(0x09);  // tab
    let ws_range_max = vdupq_n_u8(0x0D);  // carriage return
    let space_vec = vdupq_n_u8(0x20);     // space

    let mut seen_space=true;// for word counting -first char will assume text had space beofe it to count as word start
    let ones=vdupq_n_u8(1);
    let cont_mask = vdupq_n_u8(0b11000000); // Mask to check top 2 bits
    let cont_pattern = vdupq_n_u8(0b10000000); // UTF-8 continuation bytes: 0b10xxxxxx

    for chunk in chunks.by_ref(){
       let chunk_vec: uint8x16_t = vld1q_u8(chunk.as_ptr());

       // Check if chunk contains non-ASCII bytes (>= 0x80) for Unicode whitespace
       let ascii_threshold = vdupq_n_u8(0x80);
       let has_non_ascii = vcgeq_u8(chunk_vec, ascii_threshold);
       let non_ascii_count = vaddvq_u8(vandq_u8(has_non_ascii, ones)) as usize;

       if non_ascii_count > 0 {
           // Chunk has UTF-8 multi-byte sequences: use scalar for correct Unicode handling
           let chunk_count = wc_default::word_count_scalar_with_state(chunk, seen_space);
           res.lines += chunk_count.lines;
           res.chars += chunk_count.chars;
           res.words += chunk_count.words;

           // Update seen_space: check if last byte was whitespace
           // Safe: chunk is exactly CHUNK_SIZE (16 bytes) from chunks_exact
           seen_space = wc_default::is_whitespace_byte(chunk[15] as char);
       } else {
           // Pure ASCII chunk: use fast SIMD path
           // Count newlines
           let newline_cmp: uint8x16_t = vceqq_u8(chunk_vec, newline_vec);
           let mask = vandq_u8(newline_cmp, ones);
           res.lines += vaddvq_u8(mask) as usize;

           // Count characters (UTF-8 aware - skip continuation bytes 0b10xxxxxx)
           let masked = vandq_u8(chunk_vec, cont_mask);
           let is_continuation = vceqq_u8(masked, cont_pattern);
           let is_not_continuation = vmvnq_u8(is_continuation);
           let mask = vandq_u8(is_not_continuation, ones);
           res.chars += vaddvq_u8(mask) as usize;

           // Count words - ASCII whitespace only
           let in_range = vandq_u8(
               vcgeq_u8(chunk_vec, ws_range_min),
               vcleq_u8(chunk_vec, ws_range_max)
           );
           let is_space = vceqq_u8(chunk_vec, space_vec);
           let is_ws = vorrq_u8(in_range, is_space);
           let is_not_ws = vmvnq_u8(is_ws);

           // Create "previous byte" vector by shifting
           let prev_byte_val = if seen_space { 0x00u8 } else { 0xFFu8 };
           let prev_vec = vdupq_n_u8(prev_byte_val);
           let prev_is_not_ws = vextq_u8(prev_vec, is_not_ws, 15);

           // Find word starts
           let prev_is_ws = vmvnq_u8(prev_is_not_ws);
           let word_starts = vandq_u8(is_not_ws, prev_is_ws);
           let mask = vandq_u8(word_starts, ones);
           res.words += vaddvq_u8(mask) as usize;

           // Update seen_space for next chunk
           let mut last_bytes = [0u8; 16];
           vst1q_u8(last_bytes.as_mut_ptr(), is_not_ws);
           seen_space = last_bytes[15] == 0x00;
       }
    }
    let buf = chunks.remainder();
    if !buf.is_empty() {
        let buf_count = wc_default::word_count_scalar_with_state(buf, seen_space);
        res.chars += buf_count.chars;
        res.lines += buf_count.lines;
        res.words += buf_count.words;
    };

    res
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
