


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
    
    let ones=vdupq_n_u8(1);
    let cont_vec=vdupq_n_u8(0b11000000); //continuation byte mask
    for chunk in chunks.by_ref(){
       let chunk: uint8x16_t =   vld1q_u8( chunk.as_ptr());  
       let newline_cmp: uint8x16_t = vceqq_u8(chunk, newline_vec);
       let mask = vandq_u8(newline_cmp, ones);        // convert 0xFF → 1
       res.lines += vaddvq_u8(mask) as usize;     // sum lanes 
       let cont_cmp: uint8x16_t =vandq_u8(chunk, cont_vec);// mask out top two bits 
       let cont_cmp: uint8x16_t = vceqq_u8(cont_cmp, cont_vec); // 0xFF for continuation
       let letters_cmp=vmvnq_u8(cont_cmp); // invert - now 0xFF for non-continuation bytes
       let mask = vandq_u8(letters_cmp, ones);        // convert 0xFF → 1
       res.chars += vaddvq_u8(mask) as usize;     // sum lanes
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
