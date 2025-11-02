use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use std::io::{self, Read};
use std::path::PathBuf;

mod wc_x86;

/// File statistics for word count operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileCounts {
    pub lines: usize,
    pub words: usize,
    pub bytes: usize,
    pub chars: usize,
}

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
    if let Err(e) = run() {
        eprintln!("wc-rs: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = WordCountArgs::parse();

    // Set default flags if none are specified
    if !args.lines && !args.words && !args.bytes && !args.chars {
        args.lines = true;
        args.words = true;
        args.bytes = true;
    }

    if args.files.is_empty() {
        process_stdin(&args)?;
    } else {
        process_files(&args)?;
    }

    Ok(())
}

fn process_stdin(args: &WordCountArgs) -> Result<()> {
    let content = read_stdin()?;
    let stats = count_text(&content);
    print_stats(&stats, args, None);
    Ok(())
}

fn process_files(args: &WordCountArgs) -> Result<()> {
    for file_path in &args.files {
        let content = std::fs::read_to_string(file_path)
            .with_context(|| format!("failed to read file '{}'", file_path.display()))?;
        let stats = count_text(&content);
        print_stats(&stats, args, Some(file_path));
    }
    Ok(())
}

fn print_stats(stats: &FileCounts, args: &WordCountArgs, file_path: Option<&PathBuf>) {
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

    match file_path {
        Some(path) => println!("{}", path.display()),
        None => println!(),
    }
}

fn read_stdin() -> Result<String> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("failed to read from stdin")?;
    Ok(buffer)
}

/// Count text statistics using the fastest available method (SIMD or scalar)
fn count_text(content: &str) -> FileCounts {
    // Try SIMD first, fallback to scalar implementation
    if let Some(simd_result) = wc_x86::count_text_simd(content.as_bytes()) {
        return simd_result;
    }

    // Fallback to scalar implementation
    count_scalar(content)
}

/// Scalar implementation for platforms without SIMD support
fn count_scalar(content: &str) -> FileCounts {
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

    FileCounts {
        lines,
        words,
        bytes: content.len(),
        chars,
    }
}
