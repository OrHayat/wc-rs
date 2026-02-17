fn main() {
    // Declare custom cfg flag for SVE availability
    println!("cargo::rustc-check-cfg=cfg(sve_available)");

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_arch == "aarch64" {
        build_sve_if_available();
    }

    generate_build_info();
}

/// Generate build information for --version flag
fn generate_build_info() {
    // Git commit hash
    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Git dirty status
    let git_dirty = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    let git_info = if git_dirty {
        format!("{}-dirty", git_hash)
    } else {
        git_hash
    };

    // Build date
    let build_date = chrono_lite_date();

    // Target triple
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_GIT_HASH={}", git_info);
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);
    println!("cargo:rustc-env=BUILD_TARGET={}", target);

    // Rerun if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
}

/// Simple date without external crate
fn chrono_lite_date() -> String {
    std::process::Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Attempts to build SVE C code if the toolchain supports it
fn build_sve_if_available() {
    println!("cargo:rerun-if-changed=src/lib/wc_arm64_sve.c");

    let mut build = cc::Build::new();
    build.file("src/lib/wc_arm64_sve.c");

    // Test if this compiler can compile SVE before proceeding
    if !can_compile_sve(&build.get_compiler()) {
        println!(
            "cargo:warning=ARM64 detected but cannot compile SVE (missing headers or toolchain support)"
        );
        return;
    }

    println!("cargo:warning=SVE headers available - compiling SVE C code");

    build.flag("-march=armv8.2-a+sve");

    let coverage_enabled = is_coverage_enabled();

    if coverage_enabled {
        configure_coverage(&mut build);
    } else {
        build.flag("-O3");
    }

    build.compile("wc_arm64_sve");

    // Tell downstream crates to link this library
    println!("cargo:rustc-link-lib=static=wc_arm64_sve");

    // Enable the 'sve_available' cfg flag so Rust code knows SVE was built
    println!("cargo:rustc-cfg=sve_available");

    if coverage_enabled {
        link_coverage_libraries(&build);
    }
}

/// Checks if coverage instrumentation is enabled
fn is_coverage_enabled() -> bool {
    std::env::var("CARGO_LLVM_COV").is_ok()
        || std::env::var("RUSTFLAGS")
            .unwrap_or_default()
            .contains("instrument-coverage")
        || std::env::var("RUSTFLAGS")
            .unwrap_or_default()
            .contains("profile-generate")
}

/// Configures coverage instrumentation flags based on the compiler
fn configure_coverage(build: &mut cc::Build) {
    println!("cargo:warning=Building C code with coverage instrumentation");

    let compiler = build.get_compiler();

    if compiler.is_like_clang() {
        // Clang: use LLVM coverage instrumentation
        build
            .flag("-fprofile-instr-generate")
            .flag("-fcoverage-mapping");
        println!("cargo:warning=Using Clang LLVM coverage instrumentation");
    } else if compiler.is_like_gnu() {
        // GCC: use gcov-style coverage
        build.flag("--coverage");
        println!("cargo:warning=Using GCC gcov coverage instrumentation");
    } else {
        // Unknown compiler: try LLVM style as fallback
        build
            .flag("-fprofile-instr-generate")
            .flag("-fcoverage-mapping");
        println!("cargo:warning=Unknown compiler, trying LLVM coverage instrumentation");
    }
}

/// Links coverage libraries when needed (GCC on Linux requires gcov)
fn link_coverage_libraries(build: &cc::Build) {
    let compiler = build.get_compiler();

    if !compiler.is_like_gnu() {
        return; // Only GCC needs gcov library
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        println!("cargo:rustc-link-search=native=/usr/lib/gcc/aarch64-linux-gnu/13");
        println!("cargo:rustc-link-lib=static=gcov");
    }
}

/// Tests if the compiler can compile SVE code
fn can_compile_sve(compiler: &cc::Tool) -> bool {
    use std::io::Write;

    let test_code = r#"
#ifdef __ARM_FEATURE_SVE
#include <arm_sve.h>
int main() {
    svbool_t pg = svptrue_b8();
    return 0;
}
#else
#error "SVE not supported"
#endif
"#;

    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let test_file = format!("{}/test_sve.c", out_dir);
    let test_obj = format!("{}/test_sve.o", out_dir);

    // Write test file
    let Ok(mut file) = std::fs::File::create(&test_file) else {
        return false;
    };

    if file.write_all(test_code.as_bytes()).is_err() {
        return false;
    }

    // Use the same compiler that will be used for the actual build

    let result = compiler
        .to_command()
        .args(["-march=armv8.2-a+sve", "-c", &test_file, "-o", &test_obj])
        .output();

    // Clean up
    let _ = std::fs::remove_file(&test_file);
    let _ = std::fs::remove_file(&test_obj);

    result
        .map(|output| output.status.success())
        .unwrap_or(false)
}
