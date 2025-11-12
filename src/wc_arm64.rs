


#[cfg(target_arch = "aarch64")]
use  std::arch::aarch64::*;
use crate::FileCounts;
use crate::wc_default;


#[cfg(target_arch = "aarch64")]
pub fn count_text_simd(content: &[u8]) -> Option<FileCounts> {
    if std::arch::is_aarch64_feature_detected!("neon") {
        return  Some(unsafe {
            count_text_neon(content)
        })
    }
    None
}

//         let simd_result = unsafe { count_text_avx512(content) };
//         return Some(FileCounts {


unsafe  fn  count_text_neon(content: &[u8]) -> FileCounts{
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
    let space_vec =  vdupq_n_u8(b' ');
    let tab_vec = vdupq_n_u8(b'\t');
    let cr_vec =  vdupq_n_u8(b'\r');
    let ff_vec = vdupq_n_u8(0x0C);
    let vt_vec = vdupq_n_u8(0x0B);
    
    let mut seen_space=true;// for word counting -first char will assume text had space beofe it to count as word start
    let ones=vdupq_n_u8(1);
    let cont_vec=vdupq_n_u8(0b11000000); //continuation byte mask
    for chunk in chunks.by_ref(){
       let chunk: uint8x16_t =   vld1q_u8( chunk.as_ptr());

       // Count newlines
       let newline_cmp: uint8x16_t = vceqq_u8(chunk, newline_vec);
       let mask = vandq_u8(newline_cmp, ones);        // convert 0xFF → 1
       res.lines += vaddvq_u8(mask) as usize;     // sum lanes

       // Count characters (UTF-8 aware - skip continuation bytes)
       let cont_cmp: uint8x16_t =vandq_u8(chunk, cont_vec);// mask out top two bits
       let cont_cmp: uint8x16_t = vceqq_u8(cont_cmp, cont_vec); // 0xFF for continuation
       let letters_cmp=vmvnq_u8(cont_cmp); // invert - now 0xFF for non-continuation bytes
       let mask = vandq_u8(letters_cmp, ones);        // convert 0xFF → 1
       res.chars += vaddvq_u8(mask) as usize;     // sum lanes

       // Count words - detect transitions from whitespace to non-whitespace
       // Step 1: Compare chunk against each whitespace character
       let space_cmp = vceqq_u8(chunk, space_vec);     // 0xFF where byte is space
       let tab_cmp = vceqq_u8(chunk, tab_vec);         // 0xFF where byte is tab
       let newline_cmp = vceqq_u8(chunk, newline_vec); // 0xFF where byte is newline
       let cr_cmp = vceqq_u8(chunk, cr_vec);           // 0xFF where byte is carriage return
       let ff_cmp = vceqq_u8(chunk, ff_vec);           // 0xFF where byte is form feed
       let vt_cmp = vceqq_u8(chunk, vt_vec);           // 0xFF where byte is vertical tab

       // Step 2: Combine all whitespace comparisons with OR
       // A byte is whitespace if ANY comparison matched
       let ws1 = vorrq_u8(space_cmp, tab_cmp);
       let ws2 = vorrq_u8(newline_cmp, cr_cmp);
       let ws3 = vorrq_u8(ff_cmp, vt_cmp);
       let is_ws = vorrq_u8(vorrq_u8(ws1, ws2), ws3); // 0xFF = whitespace, 0x00 = non-whitespace

       // Step 3: Invert to get non-whitespace mask (0xFF = non-whitespace, 0x00 = whitespace)
       let is_not_ws = vmvnq_u8(is_ws);

       // Step 4: Create "previous byte" vector by shifting
       // vextq_u8(a, b, n) concatenates [a, b] and extracts starting at byte n
       // We want: [prev_state, byte0, byte1, ..., byte14]
       let prev_byte_val = if seen_space { 0x00u8 } else { 0xFFu8 };
       let prev_vec = vdupq_n_u8(prev_byte_val);
       // Extract from position 15 of prev_vec (last byte) + bytes 0-14 of is_not_ws
       let prev_is_not_ws = vextq_u8(prev_vec, is_not_ws, 15);

       // Step 5: Find word starts: current is non-whitespace AND previous was whitespace
       // word_start = is_not_ws AND NOT(prev_is_not_ws)
       let prev_is_ws = vmvnq_u8(prev_is_not_ws);  // invert to get "previous was whitespace"
       let word_starts = vandq_u8(is_not_ws, prev_is_ws);  // both conditions must be true

       // Step 6: Count word starts by summing the mask
       let mask = vandq_u8(word_starts, ones);  // convert 0xFF → 1
       res.words += vaddvq_u8(mask) as usize;

       // Step 7: Update seen_space for next chunk (check last byte of current chunk)
       // Extract last byte (byte 15) of is_not_ws
       let mut last_bytes = [0u8; 16];
       vst1q_u8(last_bytes.as_mut_ptr(), is_not_ws);
       seen_space = last_bytes[15] == 0x00;  // 0x00 means last byte was whitespace
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
}


// unsafe fn extract_mask_neon(cmp:uint8x16_t)->u16{
//     unsafe {
//     let low=vget_low_u8(cmp);
//     let high=vget_high_u8(cmp);
    
//     }
// }

pub fn print_u8x16(v: uint8x16_t, name: &str) {
    let mut arr = [0u8; 16];
    unsafe { vst1q_u8(arr.as_mut_ptr(), v) }
    println!("{} = {:?}", name, arr);
}

pub fn print_u16x8(v: uint16x8_t, name: &str) {
    let mut arr = [0u16; 8];
    unsafe { vst1q_u16(arr.as_mut_ptr(), v) }
    println!("{} = {:?}", name, arr);
}

pub fn print_u32x4(v: uint32x4_t, name: &str) {
    let mut arr = [0u32; 4];
    unsafe { vst1q_u32(arr.as_mut_ptr(), v) }
    println!("{} = {:?}", name, arr);
}

pub fn print_u64x2(v: uint64x2_t, name: &str) {
    let mut arr = [0u64; 2];
    unsafe { vst1q_u64(arr.as_mut_ptr(), v) }
    println!("{} = {:?}", name, arr);
}
