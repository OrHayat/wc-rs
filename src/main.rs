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
        if is_x86_feature_detected!("avx512bw") {
            return unsafe { count_lines_avx512(content) };
        } else if is_x86_feature_detected!("avx2") {
            return unsafe { count_lines_avx2(content) };
        } else if is_x86_feature_detected!("sse2") {
            return unsafe { count_lines_sse2(content) };
        }
    }

    // Fallback for non-x86_64 or when SIMD not available
    count_lines_scalar(content)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512bw")]
unsafe fn count_lines_avx512(content: &[u8]) -> usize {
    let mut lines = 0;
    let newline_vec = _mm512_set1_epi8(b'\n' as i8);

    let chunks = content.len() / 64;
    let mut i = 0;

    // Process 64-byte chunks with AVX-512
    for _ in 0..chunks {
        unsafe {
            let chunk = _mm512_loadu_si512(content.as_ptr().add(i) as *const __m512i);
            let newline_cmp = _mm512_cmpeq_epi8_mask(chunk, newline_vec);
            lines += newline_cmp.count_ones() as usize;
        }
        i += 64;
    }

    // Handle remaining bytes
    for &byte in &content[i..] {
        if byte == b'\n' {
            lines += 1;
        }
    }

    // If the file doesn't end with a newline but has content, count the last line
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    lines
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn count_lines_avx2(content: &[u8]) -> usize {
    let mut lines = 0;
    let newline_vec = _mm256_set1_epi8(b'\n' as i8);

    let chunks = content.len() / 32;
    let mut i = 0;

    // Process 32-byte chunks with AVX2
    for _ in 0..chunks {
        unsafe {
            let chunk = _mm256_loadu_si256(content.as_ptr().add(i) as *const __m256i);
            let newline_cmp = _mm256_cmpeq_epi8(chunk, newline_vec);
            let newline_mask = _mm256_movemask_epi8(newline_cmp) as u32;
            lines += newline_mask.count_ones() as usize;
        }
        i += 32;
    }

    // Handle remaining bytes
    for &byte in &content[i..] {
        if byte == b'\n' {
            lines += 1;
        }
    }

    // If the file doesn't end with a newline but has content, count the last line
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    lines
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn count_lines_sse2(content: &[u8]) -> usize {
    let mut lines = 0;
    let newline_vec = _mm_set1_epi8(b'\n' as i8);

    let chunks = content.len() / 16;
    let mut i = 0;

    // Process 16-byte chunks with SSE2
    for _ in 0..chunks {
        unsafe {
            let chunk = _mm_loadu_si128(content.as_ptr().add(i) as *const __m128i);
            let newline_cmp = _mm_cmpeq_epi8(chunk, newline_vec);
            let newline_mask = _mm_movemask_epi8(newline_cmp) as u16;
            lines += newline_mask.count_ones() as usize;
        }
        i += 16;
    }

    // Handle remaining bytes
    for &byte in &content[i..] {
        if byte == b'\n' {
            lines += 1;
        }
    }

    // If the file doesn't end with a newline but has content, count the last line
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    lines
}

fn count_lines_scalar(content: &[u8]) -> usize {
    let mut lines = 0;

    for &byte in content {
        if byte == b'\n' {
            lines += 1;
        }
    }

    // If the file doesn't end with a newline but has content, count the last line
    if !content.is_empty() && !content.ends_with(&[b'\n']) {
        lines += 1;
    }

    lines
}

fn count_words_and_chars(content: &str) -> (usize, usize) {
    let mut words = 0;
    let mut chars = 0;
    let mut in_word = false;

    for ch in content.chars() {
        chars += 1;

        if ch.is_whitespace() {
            in_word = false;
        } else if !in_word {
            words += 1;
            in_word = true;
        }
    }

    (words, chars)
}
