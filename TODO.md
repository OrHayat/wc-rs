# wc-rs TODO List (Consolidated)

## Completed Features

### Core Features
- [x] LICENSE file added
- [x] Stdin `-` handling fixed
- [x] README.md created
- [x] `-L` / `--max-line-length` flag implemented
- [x] Cargo.toml metadata added
- [x] `--files0-from=FILE` support
- [x] Multi-file totals with "total" line
- [x] Encoding refactoring (LocaleEncoding::C → SingleByte)
- [x] Multi-threaded file processing with rayon (configurable thread pool)
- [x] `--debug` flag showing SIMD path used
- [x] Enhanced `--version` with build info (git hash, date, target)
- [x] Clear error messages for all error cases

### Infrastructure
- [x] Benchmarking suite vs GNU wc
- [x] Integration test suite
- [x] Man page
- [x] CHANGELOG.md
- [x] Binary releases automation
- [x] Published to crates.io
- [x] Library/binary crate split
- [x] Package manager support
- [x] Performance documentation

---

## Remaining Tasks

### P1 - High Priority

#### Testing Improvements
- [ ] Large file handling tests (>2GB)
- [ ] Binary file tests
- [ ] Special files tests (/dev/null, pipes, sockets)
- [ ] Permission error tests
- [ ] Symlink tests
- [ ] Concurrent file access tests
- [ ] Comparison tests against real GNU wc output
- [ ] Benchmark speedup vs serial processing
- [ ] Add single-threaded fast path for small files (avoid Rayon overhead)
- [ ] Consider memory mapping vs read() for performance

#### Fuzzing Tests
- [ ] Set up cargo-fuzz or afl.rs
- [ ] Fuzz UTF-8 decoding logic
- [ ] Fuzz word boundary detection
- [ ] Fuzz SIMD implementations
- [ ] Fuzz with random byte sequences

#### Error Handling & Robustness
- [ ] Handle very large files that might overflow counters
- [ ] Graceful degradation when SIMD fails
- [ ] Proper exit codes documentation
- [ ] Signal handling (SIGPIPE, SIGINT)
- [ ] Memory limit handling for very large files

### P2 - Medium Priority

#### Help Improvements
- [ ] Review `--help` output for completeness
- [ ] Add examples to help text
- [ ] Document locale support in help
- [ ] Match GNU-style `--help` formatting

#### CI/CD Enhancements
- [ ] Multi-platform testing (Linux, macOS, Windows)
- [ ] Architecture testing (x86_64, aarch64)
- [ ] Code coverage reporting
- [ ] Clippy/formatting checks
- [ ] Security audits (cargo-audit)

#### Documentation
- [ ] CONTRIBUTING.md
- [ ] CODE_OF_CONDUCT.md
- [ ] Architecture documentation (when to use which SIMD path)
- [ ] API documentation for public types
- [ ] Examples for contributors

### P3 - Nice to Have

#### Distribution & Packaging
- [ ] Homebrew formula
- [ ] apt/deb package
- [ ] Docker image
- [ ] Installation script

#### Compatibility & Standards
- [ ] `--total={auto,always,never}` option (GNU wc feature)
- [ ] Verify exact output formatting matches POSIX
- [ ] Tab alignment verification vs GNU wc

#### Observability
- [ ] Statistics on parallel performance

---

## Notes

- All 792+ tests passing
- SIMD optimizations working for SSE2, AVX2, AVX512 (x86) and NEON, SVE (ARM64)
- Current locale support: C/POSIX and UTF-8
- Invalid UTF-8 support complete (scalar + SIMD)

---

## Competitive Analysis

### vs GNU wc:
- Faster (with SIMD)
- Parallel processing
- Feature parity achieved for core flags
- Properly packaged and documented

### Current Status:
- Production-ready for most use cases
- Can replace GNU wc in most scenarios
- Published and discoverable on crates.io
