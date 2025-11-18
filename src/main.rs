use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use rayon::prelude::*;
use std::io::{self, Read};
use std::path::PathBuf;

#[cfg(target_arch = "aarch64")]
mod wc_arm64;
#[cfg(all(test, target_arch = "aarch64"))]
mod wc_arm64_test;
mod wc_default;
#[cfg(test)]
mod wc_default_test;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod wc_x86;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod wc_x86_test;

/// File statistics for word count operations
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileCounts {
    pub lines: usize,
    pub words: usize,
    pub bytes: usize,
    pub chars: usize,
}

/// Locale encoding type for character handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LocaleEncoding {
    /// Single-byte encoding (C/POSIX, Latin-1, ISO-8859-*) - byte-based, ASCII whitespace only
    SingleByte,
    /// UTF-8 locale - Unicode aware, multi-byte characters
    Utf8,
}

/// Detect locale encoding from environment variables (LC_ALL, LC_CTYPE, LANG)
fn detect_locale() -> LocaleEncoding {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_CTYPE"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();

    let locale_upper = locale.to_uppercase();

    // Check for single-byte encodings: C/POSIX, Latin-1, ISO-8859-*
    if locale == "C"
        || locale == "POSIX"
        || locale_upper.contains("LATIN1")
        || locale_upper.contains("LATIN-1")
        || locale_upper.contains("ISO-8859")
        || locale_upper.contains("ISO8859") {
        LocaleEncoding::SingleByte
    } else {
        // Default to UTF-8 for all other locales
        LocaleEncoding::Utf8
    }
}

/// Determine the number of threads to use for parallel processing
/// - None: use min(4, num_cpus) - default
/// - Some(0): use all CPUs
/// - Some(n): use exactly n threads
fn determine_thread_count(num_threads: Option<usize>) -> usize {
    match num_threads {
        None => {
            // Default: min(4, num_cpus)
            let cpus = num_cpus::get();
            cpus.min(4)
        }
        Some(0) => {
            // -j or -j 0: use all CPUs
            num_cpus::get()
        }
        Some(n) => {
            // -j N: use exactly N threads
            n
        }
    }
}

/// Build version string with build info (semver format)
fn version_string() -> &'static str {
    concat!(
        env!("CARGO_PKG_VERSION"),
        "+",
        env!("BUILD_GIT_HASH"),
        ".",
        env!("BUILD_DATE"),
        ".",
        env!("BUILD_TARGET")
    )
}

#[derive(Parser, Debug)]
#[command(
    version = version_string(),
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

    /// Number of threads for parallel file processing
    /// Default: min(4, num_cpus)
    /// -j without value or -j 0: use all CPUs
    /// -j N: use N threads
    #[arg(
        short = 'j',
        long = "num-threads",
        value_name = "N",
        require_equals = true,
        num_args = 0..=1,
        default_missing_value = "0",
        value_parser = clap::value_parser!(usize)
    )]
    num_threads: Option<usize>,

    /// Read input from the files specified by NUL-terminated names in file F
    #[arg(long = "files0-from", value_name = "F", value_hint = clap::ValueHint::FilePath)]
    files0_from: Option<PathBuf>,

    /// Input files; use '-' for stdin. If empty, read from stdin.
    #[arg(value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
    files: Vec<PathBuf>,
}

fn main() {
    if let Err(_) = run() {
        // Errors already printed to stderr
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

    // Handle files0-from option
    if let Some(ref files0_path) = args.files0_from {
        if !args.files.is_empty() {
            eprintln!("wc-rs: file operands cannot be combined with --files0-from");
            return Err(anyhow::anyhow!("invalid arguments"));
        }
        args.files = read_files0_from(files0_path)?;
    }

    // Detect locale once at startup
    let locale = detect_locale();

    // Determine thread count for parallel processing
    let thread_count = determine_thread_count(args.num_threads);

    if args.files.is_empty() {
        process_stdin(&args, locale)?;
    } else {
        process_files(&args, locale, thread_count)?;
    }

    Ok(())
}

fn process_stdin(args: &WordCountArgs, locale: LocaleEncoding) -> Result<()> {
    let content = read_stdin()?;
    let stats = count_text(&content, locale);
    print_stats(&stats, args, None);
    Ok(())
}

fn process_files(args: &WordCountArgs, locale: LocaleEncoding, thread_count: usize) -> Result<()> {
    // Configure rayon thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build()
        .context("failed to build thread pool")?
        .install(|| {
            // Process files in parallel, preserving order
            // Returns Result<FileCounts> for success, or error message for failure
            let results: Vec<Result<FileCounts>> = args.files
                .par_iter()
                .map(|file_path| {
                    // Check if this is stdin
                    let content = if file_path.to_str() == Some("-") {
                        read_stdin()
                    } else {
                        std::fs::read(file_path)
                            .with_context(|| format!("{}", file_path.display()))
                    };

                    match content {
                        Ok(content) => {
                            let stats = count_text(&content, locale);
                            Ok(stats)
                        }
                        Err(e) => {
                            // Print error to stderr and continue
                            eprintln!("wc-rs: {}: {}", file_path.display(), e);
                            Err(anyhow::anyhow!("failed to read file"))
                        }
                    }
                })
                .collect();

            // Calculate totals and track errors
            let mut total = FileCounts {
                lines: 0,
                words: 0,
                bytes: 0,
                chars: 0,
            };
            let mut had_errors = false;

            // Print results in original order
            for (i, result) in results.iter().enumerate() {
                match result {
                    Ok(stats) => {
                        total.lines += stats.lines;
                        total.words += stats.words;
                        total.bytes += stats.bytes;
                        total.chars += stats.chars;

                        print_stats(stats, args, Some(&args.files[i]));
                    }
                    Err(_) => {
                        had_errors = true;
                        // Error already printed to stderr in the map
                    }
                }
            }

            // Print total line if multiple files
            if args.files.len() > 1 {
                print_total(&total, args);
            }

            // Return error if any files failed
            if had_errors {
                Err(anyhow::anyhow!("one or more files failed to process"))
            } else {
                Ok(())
            }
        })
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

fn print_total(stats: &FileCounts, args: &WordCountArgs) {
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
    println!("total");
}

fn read_stdin() -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    io::stdin()
        .read_to_end(&mut buffer)
        .context("failed to read from stdin")?;
    Ok(buffer)
}

/// Read NUL-terminated filenames from a file (for --files0-from option)
fn read_files0_from(path: &PathBuf) -> Result<Vec<PathBuf>> {
    let content = if path.to_str() == Some("-") {
        // Read from stdin
        read_stdin()?
    } else {
        // Read from file
        std::fs::read(path)
            .with_context(|| format!("failed to read from '{}'", path.display()))?
    };

    // Split on NUL characters and convert to PathBufs
    let files: Vec<PathBuf> = content
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| {
            // Convert bytes to PathBuf
            #[cfg(unix)]
            {
                use std::{os::unix::ffi::OsStrExt};
                PathBuf::from(std::ffi::OsStr::from_bytes(s))
            }
            #[cfg(not(unix))]
            {
                // On non-Unix systems, assume UTF-8
                PathBuf::from(String::from_utf8_lossy(s).to_string())
            }
        })
        .collect();

    Ok(files)
}

/// Count text statistics using the fastest available method (SIMD or scalar)
fn count_text(content: &[u8], locale: LocaleEncoding) -> FileCounts {
    // Try SIMD first based on architecture
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if let Some(simd_result) = wc_x86::count_text_simd(content, locale) {
            return simd_result;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if let Some(simd_result) = wc_arm64::count_text_simd(content, locale) {
            return simd_result;
        }
    }

    // Fallback to scalar implementation
    wc_default::word_count_scalar(content, locale)
}
