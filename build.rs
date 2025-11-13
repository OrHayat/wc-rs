fn main() {
    // Only compile SVE C code when building for ARM64 and compiler can compile SVE
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_arch == "aarch64" {
        // Check if compiler can compile SVE code (has headers and toolchain support)
        if can_compile_sve() {
            println!("cargo:rerun-if-changed=src/wc_arm64_sve.c");

            let mut build = cc::Build::new();
            build
                .file("src/wc_arm64_sve.c")
                .flag("-march=armv8.2-a+sve");  // Enable SVE instructions

            // Add coverage flags when building with cargo llvm-cov
            // Check for CARGO_LLVM_COV or RUSTFLAGS containing coverage flags
            let coverage_enabled = std::env::var("CARGO_LLVM_COV").is_ok()
                || std::env::var("RUSTFLAGS").unwrap_or_default().contains("instrument-coverage")
                || std::env::var("RUSTFLAGS").unwrap_or_default().contains("profile-generate");

            if coverage_enabled {
                println!("cargo:warning=Building C code with coverage instrumentation");

                // Platform-specific coverage flags
                let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

                if target_os == "linux" {
                    // Linux with GCC: use gcov-style coverage
                    build.flag("--coverage");  // Shorthand for -fprofile-arcs -ftest-coverage
                } else {
                    // macOS/other with Clang: use LLVM coverage
                    build.flag("-fprofile-instr-generate");
                    build.flag("-fcoverage-mapping");
                }
            } else {
                build.flag("-O3");  // Optimize for performance (only when not doing coverage)
            }

            build.compile("wc_arm64_sve");

            // Link with gcov when coverage is enabled on Linux
            if coverage_enabled {
                let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
                if target_os == "linux" {
                    // Add gcc library path and link with gcov on Linux
                    println!("cargo:rustc-link-search=native=/usr/lib/gcc/aarch64-linux-gnu/13");
                    println!("cargo:rustc-link-lib=static=gcov");
                }
            }

            println!("cargo:warning=SVE headers available - compiling SVE C code");

            // Tell Rust code that SVE build is available
            println!("cargo:rustc-cfg=feature=\"sve\"");
        } else {
            println!("cargo:warning=ARM64 detected but cannot compile SVE (missing headers or toolchain support)");
        }
    }
}

fn can_compile_sve() -> bool {
    use std::io::Write;
    use std::process::Command;

    // Create a temporary test file
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

    // Write test file
    if let Ok(mut file) = std::fs::File::create(&test_file) {
        if file.write_all(test_code.as_bytes()).is_err() {
            return false;
        }
    } else {
        return false;
    }

    // Try to compile it
    let result = Command::new("cc")
        .arg("-march=armv8.2-a+sve")
        .arg("-c")
        .arg(&test_file)
        .arg("-o")
        .arg(format!("{}/test_sve.o", out_dir))
        .output();

    // Clean up
    let _ = std::fs::remove_file(&test_file);
    let _ = std::fs::remove_file(format!("{}/test_sve.o", out_dir));

    result.map(|output| output.status.success()).unwrap_or(false)
}
