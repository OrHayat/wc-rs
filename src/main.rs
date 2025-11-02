use anyhow::{Context, Result};
use std::io::{self, Read};
use std::path::PathBuf;

use clap::{ArgAction, Parser};

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Print newline, word, and byte counts for each FILE.",
    long_about = r#"Print newline, word, and byte counts for each FILE, and a total line if more than one FILE is specified.
A word is a non-zero-length sequence of printable characters delimited by white space."#
)]
struct WordCountArgs {
    /// Print the newline counts
    #[arg(short = 'l', long = "lines", action = ArgAction::SetTrue)]
    lines: bool,
    /// Print the word counts
    #[arg(short = 'w', long = "words", action = ArgAction::SetTrue)]
    words: bool,

    /// Print the byte counts
    #[arg(short = 'c', long = "bytes", action = ArgAction::SetTrue)]
    bytes: bool,

    /// Print the character counts (multi-byte aware)
    #[arg(short = 'm', long = "chars", action = ArgAction::SetTrue)]
    chars: bool,

    /// Input files; use '-' for stdin. If empty, read from stdin.
    #[arg(value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
    files: Vec<PathBuf>,
}

fn main() {
    // Top-level error handler for clean CLI output
    if let Err(e) = run() {
        eprintln!("wc-rs: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = WordCountArgs::parse();

    // Default flags if none are set
    if !args.bytes && !args.chars && !args.lines && !args.words {
        args.lines = true;
        args.words = true;
        args.bytes = true;
    }

    if args.files.is_empty() {
        // Read from stdin
        let content = read_stdin().context("failed to read stdin")?;
        let stats = count_file_contents(content);
        dump_file_stats(&stats, &args, None);
    } else {
        // Read each file
        for file_path in &args.files {
            let file_content = std::fs::read_to_string(file_path)
                .with_context(|| format!("failed to read file '{}'", file_path.display()))?;
            let stats = count_file_contents(file_content);
            dump_file_stats(&stats, &args, Some(file_path));
        }
    }

    Ok(())
}

fn dump_file_stats(stats: &FileCounts, args: &WordCountArgs, file_path: Option<&PathBuf>) {
    if args.lines {
        print!("{}\t", stats.lines);
    }
    if args.words {
        print!("{}\t", stats.words);
    }
    if args.bytes {
        print!("{}\t", stats.bytes);
    }
    if args.chars {
        print!("{}\t", stats.chars);
    }
    if let Some(path) = file_path {
        println!("{}", path.display());
    } else {
        println!(); // Just print a newline for stdin
    }
}

fn read_stdin() -> Result<String, std::io::Error> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer)
}
struct FileCounts {
    lines: usize,
    words: usize,
    bytes: usize,
    chars: usize,
}

fn count_file_contents(file_content: String) -> FileCounts {
    let bytes = file_content.len();

    // Use SIMD for newline counting, handle UTF-8 properly for chars/words
    let lines = count_lines_simd(file_content.as_bytes());
    let (words, chars) = count_words_and_chars(&file_content);

    FileCounts {
        lines,
        words,
        bytes,
        chars,
    }
}

fn count_lines_simd(content: &[u8]) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        // Runtime CPU feature detection - picks the best available SIMD instruction set
        // AVX-512: 64 bytes/instruction (Intel Skylake-X+, Ice Lake+, some AMD Zen 4+)
        if is_x86_feature_detected!("avx512bw") {
            return unsafe { count_lines_avx512(content) };
        // AVX2: 32 bytes/instruction (Intel Haswell+, AMD Excavator+)
        } else if is_x86_feature_detected!("avx2") {
            return unsafe { count_lines_avx2(content) };
        // SSE2: 16 bytes/instruction (almost all x86_64 CPUs)
        } else if is_x86_feature_detected!("sse2") {
            return unsafe { count_lines_sse2(content) };
        }
    }

    // Fallback for non-x86_64 architectures or ancient CPUs without SIMD
    count_lines_scalar(content)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512bw")] // Requires AVX-512 Byte/Word operations
unsafe fn count_lines_avx512(content: &[u8]) -> usize {
    let mut lines = 0;

    // Create a vector of 64 bytes, all set to '\n' (newline character)
    // This will be compared against each 64-byte chunk of the file
    let newline_vec = _mm512_set1_epi8(b'\n' as i8);

    let chunks = content.len() / 64; // How many 64-byte chunks we can process
    let mut i = 0;

    // Process file in 64-byte chunks using AVX-512
    for _ in 0..chunks {
        unsafe {
            // Load 64 bytes from memory into AVX-512 register
            // _mm512_loadu_si512 = unaligned load (file data might not be 64-byte aligned)
            let chunk = _mm512_loadu_si512(content.as_ptr().add(i) as *const __m512i);

            // Compare each of the 64 bytes in chunk with '\n'
            // Returns a 64-bit mask where bit=1 means that byte was '\n'
            let newline_cmp = _mm512_cmpeq_epi8_mask(chunk, newline_vec);

            // Count how many bits are set in the mask = how many '\n' found
            lines += newline_cmp.count_ones() as usize;
        }
        i += 64; // Move to next 64-byte chunk
    }

    // Handle leftover bytes that don't fill a complete 64-byte chunk
    // (e.g., if file is 100 bytes, we process 64 bytes with SIMD, 36 bytes normally)
    for &byte in &content[i..] {
        if byte == b'\n' {
            lines += 1;
        }
    }

    // wc behavior: if file has content but doesn't end with '\n', count it as a line
    // Example: "hello" (no newline) should count as 1 line
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    lines
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")] // Requires AVX2 (Advanced Vector Extensions 2)
unsafe fn count_lines_avx2(content: &[u8]) -> usize {
    let mut lines = 0;

    // Create a vector of 32 bytes, all set to '\n'
    let newline_vec = _mm256_set1_epi8(b'\n' as i8);

    let chunks = content.len() / 32; // How many 32-byte chunks we can process
    let mut i = 0;

    // Process file in 32-byte chunks using AVX2
    for _ in 0..chunks {
        unsafe {
            // Load 32 bytes from memory into AVX2 register (256-bit)
            let chunk = _mm256_loadu_si256(content.as_ptr().add(i) as *const __m256i);

            // Compare each of the 32 bytes in chunk with '\n'
            // Returns a 256-bit vector where each byte is 0xFF if match, 0x00 if no match
            let newline_cmp = _mm256_cmpeq_epi8(chunk, newline_vec);

            // Extract the high bit of each byte to create a 32-bit mask
            // If byte was 0xFF (match), the high bit becomes 1 in the mask
            let newline_mask = _mm256_movemask_epi8(newline_cmp) as u32;

            // Count set bits in mask = count of newlines found
            lines += newline_mask.count_ones() as usize;
        }
        i += 32; // Move to next 32-byte chunk
    }

    // Process remaining bytes (< 32 bytes) with scalar code
    for &byte in &content[i..] {
        if byte == b'\n' {
            lines += 1;
        }
    }

    // wc behavior: count final line even if it doesn't end with newline
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    lines
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")] // Requires SSE2 (available on virtually all x86_64)
unsafe fn count_lines_sse2(content: &[u8]) -> usize {
    let mut lines = 0;

    // Create a vector of 16 bytes, all set to '\n'
    let newline_vec = _mm_set1_epi8(b'\n' as i8);

    let chunks = content.len() / 16; // How many 16-byte chunks we can process
    let mut i = 0;

    // Process file in 16-byte chunks using SSE2
    for _ in 0..chunks {
        unsafe {
            // Load 16 bytes from memory into SSE register (128-bit)
            let chunk = _mm_loadu_si128(content.as_ptr().add(i) as *const __m128i);

            // Compare each of the 16 bytes in chunk with '\n'
            // Returns a 128-bit vector where each byte is 0xFF if match, 0x00 if no match
            let newline_cmp = _mm_cmpeq_epi8(chunk, newline_vec);

            // Extract the high bit of each byte to create a 16-bit mask
            // Same principle as AVX2 but with 16 bytes instead of 32
            let newline_mask = _mm_movemask_epi8(newline_cmp) as u16;

            // Count set bits in mask = count of newlines found
            lines += newline_mask.count_ones() as usize;
        }
        i += 16; // Move to next 16-byte chunk
    }

    // Process remaining bytes (< 16 bytes) with scalar code
    for &byte in &content[i..] {
        if byte == b'\n' {
            lines += 1;
        }
    }

    // wc behavior: count final line even if it doesn't end with newline
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    lines
}

fn count_lines_scalar(content: &[u8]) -> usize {
    // Simple byte-by-byte line counting for CPUs without SIMD support
    // Also used for very small files where SIMD overhead isn't worth it
    let mut lines = 0;

    for &byte in content {
        if byte == b'\n' {
            lines += 1;
        }
    }

    // wc behavior: count final line even if it doesn't end with newline
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    lines
}

fn count_words_and_chars(content: &str) -> (usize, usize) {
    // Word and character counting with proper UTF-8 support
    // Uses Rust's char iterator which correctly handles multi-byte Unicode characters
    let mut words = 0;
    let mut chars = 0;
    let mut in_word = false; // State machine: are we currently inside a word?

    for ch in content.chars() {
        // Iterates over Unicode characters, not bytes
        chars += 1;

        if ch.is_whitespace() {
            // Found whitespace: end current word (if any)
            in_word = false;
        } else if !in_word {
            // Found non-whitespace after whitespace: start new word
            words += 1;
            in_word = true;
        }
        // If in_word==true and ch is not whitespace: continue current word (no action)
    }

    (words, chars)
}
