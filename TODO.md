# wc-rs TODO List

## PROJECT ANALYSIS: Senior Product Manager & Architect Review

### CURRENT STATE - What You Have Built

**Strong Technical Foundation:**
- SIMD-optimized implementation (AVX2 for x86/x64, NEON+SVE for ARM64)
- Parallel file processing with configurable threading
- Locale-aware (C/POSIX vs UTF-8)
- Comprehensive test coverage with property-based testing
- Cross-platform architecture support with scalar fallback

---

## CRITICAL GAPS & ROADMAP

### 1. MISSING CORE FEATURES ⚠️

#### `-L` / `--max-line-length` flag
- This is a STANDARD POSIX/GNU wc feature
- Required for compatibility with existing scripts
- **Priority: HIGH**

#### ✅ Stdin handling with `-` argument
- GNU wc accepts `-` to explicitly read stdin in file list
- **Priority: CRITICAL**
- **Status: COMPLETED**

#### ✅ `--files0-from=FILE` option
- GNU wc feature for null-delimited file lists
- Important for xargs integration
- **Priority: MEDIUM**
- **Status: COMPLETED**

---

### 2. PROJECT INFRASTRUCTURE 📦

#### Missing Cargo.toml metadata:
```toml
# You have NONE of these:
description = "..."
repository = "https://github.com/..."
license = "MIT OR Apache-2.0"
authors = ["..."]
keywords = ["cli", "text", "wc", "simd"]
categories = ["command-line-utilities"]
readme = "README.md"
```
**Priority: CRITICAL**

#### No LICENSE file
- Cannot be used in production without a license
- Legal risk for any user
- **Priority: CRITICAL**

#### No README.md
- No documentation on what this is
- No installation instructions
- No performance comparisons
- No build instructions
- **Priority: CRITICAL**

---

### 3. BENCHMARKING & PERFORMANCE ⚡

#### No benchmarks directory
- You claim SIMD performance but have no proof
- No comparisons against GNU wc
- Cannot track performance regressions
- **Priority: HIGH**

**Should include:**
- Criterion benchmarks vs GNU wc
- Various file sizes (1KB, 1MB, 100MB, 1GB)
- Different content types (ASCII, UTF-8, binary)
- Parallel vs serial benchmarks

---

### 4. TESTING GAPS 🧪

#### No integration tests
- Only unit tests exist
- No end-to-end CLI testing
- No comparison tests against real GNU wc output

**Missing test scenarios:**
- Large file handling (>2GB)
- Binary files
- Special files (/dev/null, pipes, sockets)
- Permission errors
- Symlinks
- Concurrent file access

---

### 5. ERROR HANDLING & ROBUSTNESS 🛡️

**Issues found:**
- stdin `-` handling broken (main.rs:170)
- No clear error messages for unsupported options
- No handling of very large files that might overflow counters
- No graceful degradation when SIMD fails

**Missing:**
- Proper exit codes documentation
- Signal handling (SIGPIPE, SIGINT)
- Memory limit handling for very large files

---

### 6. DOCUMENTATION 📚

**Completely missing:**
- README.md with feature comparison
- CHANGELOG.md
- CONTRIBUTING.md
- CODE_OF_CONDUCT.md
- Man page
- Performance documentation
- Architecture documentation (when to use which SIMD path)
- API documentation for public types

---

### 7. DISTRIBUTION & PACKAGING 📦

**No distribution strategy:**
- No binary releases
- Not published to crates.io
- No package manager support (Homebrew, apt, etc.)
- No installation script
- No Docker image

---

### 8. OBSERVABILITY & DEBUGGING 🔍

**Missing:**
- `--debug` flag to show which SIMD path was used
- `--version` output doesn't show build info
- No way to disable SIMD for testing
- No verbose mode
- No statistics on parallel performance

---

### 9. COMPATIBILITY & STANDARDS 📋

#### GNU wc features you're missing:
- `--files0-from=F` - read input from null-delimited file list
- `-L` / `--max-line-length` - print max display width
- `--total={auto,always,never}` - control total line
- Proper `--help` formatting matching GNU style

#### POSIX compliance issues:
- Need to verify exact output formatting matches
- Tab alignment differences might exist

---

### 10. ARCHITECTURAL CONCERNS 🏗️

**Code organization:**
- No separation of CLI from library
- Cannot be used as a library crate
- Consider: `wc-rs` (binary) + `wc-core` (library)

**Performance:**
- Rayon overhead for small files?
- Should have single-threaded fast path
- Memory mapping might be faster than read()

**Maintainability:**
- Good SIMD abstractions
- Test coverage is excellent
- But missing examples for contributors

---

## PRIORITY ROADMAP

### P0 (Must Have Before v1.0)
1. ✅ Add LICENSE file
2. ✅ Fix stdin `-` handling bug (completed 2025-11-18)
3. ✅ Create comprehensive README.md
4. ✅ Implement `-L` flag
5. ✅ Add package metadata to Cargo.toml
6. ✅ Set up basic CI/CD

### P1 (Should Have)
1. ✅ Benchmarking suite vs GNU wc
2. ✅ Integration test suite
3. ✅ Man page
4. ✅ CHANGELOG.md
5. ✅ Binary releases automation
6. ✅ Publish to crates.io

### P2 (Nice to Have)
1. ✅ `--files0-from` support
2. ✅ Debug/verbose modes
3. ✅ Library/binary crate split
4. ✅ Package manager support
5. ✅ Performance documentation

---

## COMPETITIVE ANALYSIS 📊

### vs GNU wc:
- ✅ Faster (with SIMD)
- ✅ Parallel processing
- ❌ Missing `-L` flag
- ✅ `--files0-from` support
- ❌ No man page
- ❌ Not packaged anywhere

### Market Position:
- Cannot replace GNU wc yet (missing features)
- Cannot be used in production (no license)
- Cannot be discovered (no README, no crates.io)
- Cannot be trusted (no benchmarks, no CI)

---

## CI/CD & AUTOMATION 🔄
**(To be implemented AFTER repo becomes public)**

### GitHub Actions Setup:

**Missing workflows:**
- Multi-platform testing (Linux, macOS, Windows)
- Architecture testing (x86_64, aarch64)
- Automated releases
- Code coverage reporting
- Clippy/formatting checks
- Security audits (cargo-audit)

**Priority: HIGH** (once repo is public)

---

## BOTTOM LINE

You have built a **technically impressive SIMD-optimized implementation**, but it's **not production-ready** and **not distributable**. The project needs infrastructure, documentation, and completion of standard features before it can be considered a viable alternative to GNU wc.

**Next steps:** Focus on P0 items to get to a v1.0 release.

---

## CODE IMPROVEMENTS 🔧

### ✅ Encoding Support Refactoring (Completed)
- ✅ Renamed `LocaleEncoding::C` enum variant to `SingleByte` to better reflect its purpose
- ✅ Updated implementation to support mapping of multiple single-byte encodings (C/POSIX, Latin-1, ISO-8859-*) to the `SingleByte` variant
- ✅ Updated all test files and implementation to use the new enum variant name
