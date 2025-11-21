use chrono::{DateTime, Local};
use clap::Parser;
use serde::{Deserialize, Serialize};
use signal_hook::consts::SIGINT;
use signal_hook::flag;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Continuous fuzzing runner for wc-rs
#[derive(Parser, Debug)]
#[command(name = "fuzz-runner")]
#[command(about = "Run all fuzz targets continuously with statistics tracking", long_about = None)]
struct Args {
    /// Time per target per iteration in seconds (default: 60)
    #[arg(short, long, group = "mode")]
    time: Option<u64>,

    /// Run forever (no time limit per target)
    #[arg(short, long, group = "mode")]
    infinity: bool,

    /// Maximum number of iterations (default: unlimited)
    #[arg(short, long)]
    max_iterations: Option<u32>,
}

/// Statistics for a single fuzz run
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunStats {
    target: String,
    iteration: u32,
    start_time: DateTime<Local>,
    end_time: DateTime<Local>,
    duration_secs: u64,
    exit_code: i32,
    corpus_size: usize,
    crashes_found: usize,
    coverage: Option<u32>,
    exec_per_sec: Option<u32>,
}

/// Session statistics
#[derive(Debug, Serialize, Deserialize)]
struct SessionStats {
    session_start: DateTime<Local>,
    session_end: Option<DateTime<Local>>,
    total_iterations: u32,
    total_runs: u32,
    total_crashes: usize,
    max_iterations: Option<u32>,
    runs: Vec<RunStats>,
}

struct FuzzRunner {
    targets: Vec<String>,
    time_per_run: u64,
    corpus_path: PathBuf,
    log_dir: PathBuf,
    stop_signal: Arc<AtomicBool>,
    session_stats: SessionStats,
}

impl FuzzRunner {
    fn new(time_per_run: u64) -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));

        // Register signal handler
        flag::register(SIGINT, Arc::clone(&stop_signal))
            .expect("Failed to register signal handler");

        Self {
            targets: vec![
                "fuzz_word_count".to_string(),
                "fuzz_utf8".to_string(),
                "fuzz_simd_consistency".to_string(),
                "fuzz_counter_overflow".to_string(),
                "fuzz_word_boundaries".to_string(),
            ],
            time_per_run,
            corpus_path: PathBuf::from("corpus/shared"),
            log_dir: PathBuf::from("logs"),
            stop_signal,
            session_stats: SessionStats {
                session_start: Local::now(),
                session_end: None,
                total_iterations: 0,
                total_runs: 0,
                total_crashes: 0,
                max_iterations: None,
                runs: Vec::new(),
            },
        }
    }

    fn run(&mut self) {
        // Initialize corpus from seeds if empty
        self.ensure_corpus_initialized();

        // Create log directory
        fs::create_dir_all(&self.log_dir).expect("Failed to create log directory");

        let log_file = self.log_dir.join(format!(
            "fuzz_{}.log",
            self.session_stats.session_start.format("%Y%m%d_%H%M%S")
        ));
        let stats_file = self.log_dir.join(format!(
            "stats_{}.json",
            self.session_stats.session_start.format("%Y%m%d_%H%M%S")
        ));

        println!("==================================================");
        println!("🚀 Continuous Fuzzing Runner");
        println!("==================================================");
        println!("Start time: {}", self.session_stats.session_start);
        if self.time_per_run == 0 {
            println!("Mode: Continuous (no time limit per target)");
        } else {
            println!("Time per run: {}s", self.time_per_run);
        }
        println!("Targets: {}", self.targets.join(", "));
        println!("Log file: {}", log_file.display());
        println!("Stats file: {}", stats_file.display());
        println!("Press Ctrl+C to stop gracefully");
        println!("==================================================\n");

        let mut iteration = 1;

        while !self.stop_signal.load(Ordering::Relaxed) {
            println!("\n=========================================");
            println!("📊 Iteration {} - {}", iteration, Local::now().format("%H:%M:%S"));
            println!("=========================================");

            let mut compilation_failed = false;
            for target in &self.targets.clone() {
                if self.stop_signal.load(Ordering::Relaxed) {
                    println!("\n🛑 Stop signal received, finishing current iteration...");
                    break;
                }

                let stats = self.run_target(target, iteration);

                // Check for compilation failure
                if stats.exit_code != 0 && stats.coverage.is_none() && stats.crashes_found == 0 {
                    compilation_failed = true;
                }

                self.session_stats.runs.push(stats.clone());
                self.session_stats.total_runs += 1;

                // Log stats
                self.print_run_summary(&stats);

                // Save stats to file
                self.save_stats(&stats_file);

                // Stop immediately on compilation failure
                if compilation_failed {
                    eprintln!("\n🛑 Stopping due to compilation failure. Fix the errors and try again.");
                    self.stop_signal.store(true, Ordering::Relaxed);
                    break;
                }
            }

            if !self.stop_signal.load(Ordering::Relaxed) {
                println!("\n✓ Iteration {} complete", iteration);
                self.session_stats.total_iterations += 1;
                iteration += 1;

                // Check if we've hit max iterations
                if let Some(max_iter) = self.session_stats.max_iterations {
                    if iteration > max_iter {
                        println!("\n✅ Reached max iterations ({}), stopping...", max_iter);
                        break;
                    }
                }
            }
        }

        // Final summary
        self.session_stats.session_end = Some(Local::now());
        self.print_final_summary();
        self.save_stats(&stats_file);
    }

    fn run_target(&mut self, target: &str, iteration: u32) -> RunStats {
        println!("\n--- Running: {} ---", target);

        let start_time = Local::now();
        let start_instant = Instant::now();

        // Run cargo fuzz
        let mut cmd = Command::new("cargo");
        cmd.arg("fuzz")
            .arg("run")
            .arg(target)
            .arg(&self.corpus_path)
            .arg("--")
            .arg("-print_final_stats=1");

        // Only add time limit if not infinity mode
        if self.time_per_run > 0 {
            cmd.arg(format!("-max_total_time={}", self.time_per_run));
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd.spawn();
        let mut coverage = None;
        let mut exec_per_sec = None;

        match output {
            Ok(mut child) => {
                // Read output line by line to extract stats
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().flatten() {
                        // Parse coverage info
                        if line.contains("cov:") {
                            if let Some(cov_str) = line.split("cov:").nth(1) {
                                if let Some(cov_val) = cov_str.split_whitespace().next() {
                                    coverage = cov_val.parse().ok();
                                }
                            }
                        }

                        // Parse exec/s
                        if line.contains("exec/s:") {
                            if let Some(exec_str) = line.split("exec/s:").nth(1) {
                                if let Some(exec_val) = exec_str.split_whitespace().next() {
                                    exec_per_sec = exec_val.parse().ok();
                                }
                            }
                        }
                    }
                }

                let status = child.wait().expect("Failed to wait on child");
                let exit_code = status.code().unwrap_or(-1);

                let end_time = Local::now();
                let duration = start_instant.elapsed();

                // Get corpus size
                let corpus_size = self.count_corpus_files();

                // Check for crashes
                let crashes_found = self.count_crashes(target);
                if crashes_found > 0 {
                    println!("🚨 CRASH FOUND! {} crash(es) in artifacts/{}", crashes_found, target);
                    self.session_stats.total_crashes += crashes_found;
                }

                // Check if compilation/execution failed
                // If we got coverage info, the fuzzer ran successfully (even if it found crashes)
                // If exit_code != 0 AND no coverage, likely compilation failure
                if exit_code != 0 && coverage.is_none() && crashes_found == 0 {
                    eprintln!("❌ Target '{}' failed with exit code {}", target, exit_code);
                    eprintln!("   No coverage data collected - likely compilation or startup failure.");
                    eprintln!("   Try running: cargo fuzz run {}", target);
                } else if exit_code != 0 && crashes_found > 0 {
                    println!("✓ Fuzzer exited with code {} after finding crashes (expected)", exit_code);
                }

                RunStats {
                    target: target.to_string(),
                    iteration,
                    start_time,
                    end_time,
                    duration_secs: duration.as_secs(),
                    exit_code,
                    corpus_size,
                    crashes_found,
                    coverage,
                    exec_per_sec,
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to run {}: {}", target, e);
                RunStats {
                    target: target.to_string(),
                    iteration,
                    start_time,
                    end_time: Local::now(),
                    duration_secs: 0,
                    exit_code: -1,
                    corpus_size: self.count_corpus_files(),
                    crashes_found: 0,
                    coverage: None,
                    exec_per_sec: None,
                }
            }
        }
    }

    fn count_corpus_files(&self) -> usize {
        fs::read_dir(&self.corpus_path)
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    }

    fn count_crashes(&self, target: &str) -> usize {
        let artifacts_path = PathBuf::from("artifacts").join(target);
        fs::read_dir(&artifacts_path)
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    }

    fn ensure_corpus_initialized(&self) {
        let seeds_path = PathBuf::from("seeds");

        // Check if corpus is empty or doesn't exist
        let corpus_empty = match fs::read_dir(&self.corpus_path) {
            Ok(entries) => entries.count() == 0,
            Err(_) => true, // Directory doesn't exist
        };

        if corpus_empty {
            println!("📦 Corpus is empty, initializing from seeds...");

            // Create corpus directory if it doesn't exist
            if let Err(e) = fs::create_dir_all(&self.corpus_path) {
                eprintln!("⚠️  Failed to create corpus directory: {}", e);
                return;
            }

            // Check if seeds directory exists
            if !seeds_path.exists() {
                eprintln!("⚠️  Seeds directory not found at {}", seeds_path.display());
                eprintln!("   Continuing with empty corpus...");
                return;
            }

            // Copy all files from seeds to corpus
            match fs::read_dir(&seeds_path) {
                Ok(entries) => {
                    let mut copied = 0;
                    for entry in entries.filter_map(Result::ok) {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(filename) = path.file_name() {
                                let dest = self.corpus_path.join(filename);
                                if let Err(e) = fs::copy(&path, &dest) {
                                    eprintln!("⚠️  Failed to copy {}: {}", filename.to_string_lossy(), e);
                                } else {
                                    copied += 1;
                                }
                            }
                        }
                    }
                    println!("✓ Initialized corpus with {} seed files\n", copied);
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to read seeds directory: {}", e);
                    eprintln!("   Continuing with empty corpus...");
                }
            }
        }
    }

    fn print_run_summary(&self, stats: &RunStats) {
        println!("  ⏱️  Duration: {}s", stats.duration_secs);
        println!("  📦 Corpus: {} files", stats.corpus_size);
        if let Some(cov) = stats.coverage {
            println!("  📈 Coverage: {}", cov);
        }
        if let Some(eps) = stats.exec_per_sec {
            println!("  ⚡ Speed: {} exec/s", eps);
        }
        if stats.crashes_found > 0 {
            println!("  💥 Crashes: {}", stats.crashes_found);
        }
        println!("  ✓ Exit code: {}", stats.exit_code);
    }

    fn print_final_summary(&self) {
        println!("\n==================================================");
        println!("📊 Final Summary");
        println!("==================================================");
        println!("Session duration: {} - {}",
            self.session_stats.session_start.format("%H:%M:%S"),
            self.session_stats.session_end.as_ref().unwrap().format("%H:%M:%S")
        );
        println!("Total iterations: {}", self.session_stats.total_iterations);
        println!("Total runs: {}", self.session_stats.total_runs);
        println!("Total crashes found: {}", self.session_stats.total_crashes);

        if let Some(last_run) = self.session_stats.runs.last() {
            println!("Final corpus size: {} files", last_run.corpus_size);
        }

        println!("==================================================");
    }

    fn save_stats(&self, path: &PathBuf) {
        if let Ok(json) = serde_json::to_string_pretty(&self.session_stats) {
            if let Ok(mut file) = File::create(path) {
                let _ = file.write_all(json.as_bytes());
            }
        }
    }
}

fn main() {
    let args = Args::parse();

    let time_per_run = if args.infinity {
        0 // 0 means no time limit
    } else {
        args.time.unwrap_or(60) // Default to 60 if neither flag provided
    };

    println!("Configuration:");
    if args.infinity {
        println!("  Mode: Continuous (no time limit per target)");
    } else {
        println!("  Time per target: {}s", time_per_run);
    }
    if let Some(max_iter) = args.max_iterations {
        println!("  Max iterations: {}", max_iter);
    } else {
        println!("  Max iterations: unlimited");
    }
    println!();

    let mut runner = FuzzRunner::new(time_per_run);
    runner.session_stats.max_iterations = args.max_iterations;
    runner.run();
}
