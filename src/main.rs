use anyhow::{Context, Result};
use std::io::{self, Read};
use std::path::PathBuf;

use clap::{ArgAction, Parser};

mod wc_x86;

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
    if args.chars {
        print!("{}\t", stats.chars);
    }
    if args.bytes {
        print!("{}\t", stats.bytes);
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
    // Try SIMD first, fallback to regular implementation
    if let Some(simd_result) = wc_x86::count_text_simd(file_content.as_bytes()) {
        // SANITY CHECK: Compare with regular non-SIMD version
        let regular_counts = count_regular(&file_content);
        if simd_result.lines != regular_counts.0 {
            eprintln!(
                "WARNING: SIMD lines {} != regular lines {}",
                simd_result.lines, regular_counts.0
            );
        }
        if simd_result.words != regular_counts.1 {
            eprintln!(
                "WARNING: SIMD words {} != regular words {}",
                simd_result.words, regular_counts.1
            );
        }
        if simd_result.chars != regular_counts.2 {
            eprintln!(
                "WARNING: SIMD chars {} != regular chars {}",
                simd_result.chars, regular_counts.2
            );
        }

        simd_result
    } else {
        // No SIMD available, use regular implementation
        let regular_counts = count_regular(&file_content);
        FileCounts {
            lines: regular_counts.0,
            words: regular_counts.1,
            bytes: file_content.len(),
            chars: regular_counts.2,
        }
    }
}

// Regular non-SIMD implementation for comparison
fn count_regular(content: &str) -> (usize, usize, usize) {
    let mut lines = 0;
    let mut words = 0;
    let mut chars = 0;
    let mut in_word = false;

    for ch in content.chars() {
        chars += 1;

        if ch == '\n' {
            lines += 1;
        }

        if ch.is_whitespace() {
            in_word = false;
        } else if !in_word {
            words += 1;
            in_word = true;
        }
    }

    (lines, words, chars)
}
