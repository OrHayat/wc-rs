# ARM64 SIMD Optimization Research

**Goal**: Speed up `wc` (word count) text processing on ARM64 by parallelizing operations.

**The Challenge**: Default implementation processes bytes sequentially (1 byte/cycle). ARM64 offers multiple SIMD instruction sets to process 16-128+ bytes in parallel:

- **NEON** (128-bit): Universal on all ARM64. Processes 16 bytes/cycle. Available everywhere (phones, laptops, cloud).
- **Crypto Extensions**: AES/SHA instructions for fast bit manipulation. Available on most modern ARM64 (Apple Silicon, newer phones).
- **SVE** (Scalable Vector Ext): 128-2048 bit vectors, runtime-sized. Cloud/server only (AWS Graviton3). Future consumer chips may adopt.
- **SVE2**: Enhanced SVE with more instructions. Latest cloud hardware only (AWS Graviton4).

**Note**: Apple M4 added SME (Scalable Matrix Extensions) for AI/ML, but that's matrix math - not useful for text processing. We need SVE (vector ops).

**NEON-Specific Problem**: Unlike x86's `_mm_movemask_epi8`, NEON has no instruction to extract comparison results as bitmasks. Need workarounds: scalar extraction (slow), horizontal adds (packed), or table lookups (vtbl).

---

## Performance Analysis (vs Scalar Baseline)

| **Implementation**               | **Speed vs Scalar** | **Availability** | **Test Environment**| **Priority** |
|----------------------------------|---------------------|------------------|---------------------|--------------|
| **Scalar (baseline)**            | 1x                  | ✅ Universal     | All platforms       | ✅ Done      |
| **NEON (emulated movemask)**     | ~12x                | ✅ Universal     | Mac M3, all ARM64   | ✅ Done      |
| **NEON (packed movemask)**       | ~16x (est)          | ✅ Universal     | Mac M3, all ARM64   | ✅ Done      |
| **NEON + Table Lookup movemask** | ~18x (est)          | ✅ Universal     | Mac M3, all ARM64   | � Next       |
| **NEON + Crypto Extensions**     | ~20x                | ✅ Very Common   | Mac M3, most ARM64  | 📋 Planned   |
| **SVE 256-bit**                  | ~32x                | ⚠️ Cloud/Server  | AWS c7g instances   | 📋 Future    |
| **SVE2**                         | ~40-50x             | ❌ Latest Cloud  | AWS c8g instances   | 📋 Future    |

---

## Current Implementation: NEON SIMD (16 bytes/cycle)


**Implementation**: Declarative macro `generate_neon_counter!` creates identical NEON functions differing only in movemask strategy. Three variants: emulated (reference), packed (active), vtbl .

**Challenge**: NEON lacks native movemask instruction to extract comparison bitmasks (unlike x86's `_mm_movemask_epi8`).

**Movemask Solutions**: Three approaches being tested - emulated (scalar extraction, ~12x), packed (horizontal adds, ~16x), vtbl (table lookup, TBD).

**Core Logic**: Process 16 bytes in parallel with NEON comparisons → extract bitmask → count transitions.
- **UTF-8 chars**: Bytes with high bit set (≥0x80) are continuation bytes; count chars = total bytes - continuation bytes.
- **Whitespace**: NEON compares against space/tab/newline/CR simultaneously, creates mask of whitespace positions.
- **Word counting**: Count transitions in bitmask where whitespace bit → non-whitespace bit (each = new word).


### NEON Intrinsics Stability Issue: vtbl-based Movemask

Some NEON intrinsics required for optimal vtbl-based movemask (such as `vreinterpret_u8_u64`, `vtbl1_u8`) are not available in stable Rust. This limits the ability to implement the fastest movemask variant directly in Rust.

See [Rust GitHub issue #18880](https://github.com/rust-lang/rust/issues/18880) for details.

**Options to handle this limitation:**

1. **Use packed or emulated movemask variants in pure Rust:** These are fully stable and portable, but may be slightly less optimal than the vtbl-based approach.
2. **Write only the movemask function in C:** Use ARM NEON intrinsics in C for the movemask, and link it to Rust via FFI. This allows full performance for the movemask while keeping the rest of your SIMD logic in Rust.
3. **Use nightly Rust:** Enable unstable features to access more NEON intrinsics. This is a tradeoff—nightly Rust may be perfectly acceptable for apps, personal tools, or non-production use cases where users do not require long-term stability guarantees.

---

## Planned Implementation: Crypto Extensions (20x target)

**Challenge**: Use AES/SHA instructions for fast bit manipulation to accelerate bitmask operations beyond pure NEON.

**Status**: Not yet started. Requires testing on Apple Silicon (M1+) or newer ARM64 chips with Crypto Extensions.

---

## Planned Implementation: SVE/SVE2 (32-50x target)

**Challenge**: Scalable vectors process 128-2048 bits per cycle (runtime-determined width), dramatically increase parallelism.

**Status**: Cloud-only testing required (AWS Graviton3 for SVE, Graviton4 for SVE2). Consumer chips (Apple M5/M6) may add SVE in future.

---

## Hardware Compatibility

| **Device**            | **NEON** | **Crypto** | **SVE** | **SVE2** | **Notes**                    |
|-----------------------|----------|------------|---------|----------|------------------------------|
| Apple M1/M2/M3/M4     | ✅       | ✅         | ❌      | ❌       | Perfect for NEON+Crypto      |
| iPhone (A7+)          | ✅       | ✅         | ❌      | ❌       | Since iPhone 5S (2013)       |
| iPad (A7+)            | ✅       | ✅         | ❌      | ❌       | Since iPad Air (2013)        |
| Qualcomm 8cx/X Elite  | ✅       | ✅         | ❌      | ❌       | Windows on ARM laptops       |
| Samsung Exynos        | ✅       | ✅         | ❌      | ❌       | Most Android phones          |
| AWS c6g (Graviton2)   | ✅       | ✅         | ❌      | ❌       | $0.034/hr                    |
| AWS c7g (Graviton3)   | ✅       | ✅         | ✅      | ❌       | $0.036/hr                    |
| AWS c8g (Graviton4)   | ✅       | ✅         | ✅      | ✅       | $0.038/hr                    |
| Oracle Ampere Altra   | ✅       | ✅         | ⚠️      | ❌       | Variable                     |

---

## Implementation Architecture

**Tasks**:
1. ✅ Implement basic NEON (done)
2. ✅ Implement packed movemask using horizontal adds (done)
3. ✅ Create macro-based generation system (done)
4. ✅ Make packed variant active implementation (done)
5. � Implement vtbl-based movemask (next)
6. 📋 Choose fastest variant for production
7. 📋 Add crypto extensions support
8. 📋 Create comprehensive benchmark suite
9. 📋 Benchmark all variants

**Feature Check**:
```bash
sysctl -a | grep machdep.cpu.features  # Should show: AES, SHA1, SHA2
```

### **Cloud Testing (AWS Graviton)**
**Goal**: Implement and test SVE/SVE2  
**Expected Gains**: 20x → 50x performance

**Setup**:
```bash
# c7g for SVE testing
aws ec2 run-instances --image-id ami-0c2b8ca1dad447f8a --instance-type c7g.micro
cat /proc/cpuinfo | grep sve  # Check SVE support

# c8g for SVE2 testing  
aws ec2 run-instances --instance-type c8g.micro
cat /proc/cpuinfo | grep sve2  # Check SVE2 support
```

**Cost**: c7g $0.036/hr, c8g $0.038/hr (~$1 for full testing)

---

## Implementation Architecture

### **Current Structure (v0.1)**
```
wc_amd64.rs
├── count_text_neon()                      // Delegates to packed variant
├── generate_neon_counter! (macro)         // Generates variants with different movemask
│   ├── count_text_neon_packed()           // ✅ ACTIVE: horizontal adds
│   ├── count_text_neon_emulated_impl()    // ✅ REFERENCE: scalar extraction
│   └── count_text_neon_vtbl_impl()        // 🔄 TODO: vtbl-based
├── neon_movemask_u8x16_packed()           // Pure NEON (active)
├── neon_movemask_u8x16_emulated()         // Scalar (reference)
└── count_text_scalar()                    // Fallback
```

---

**Next**: Implement vtbl movemask → benchmark all 3 variants → choose best one for production

*ARM64 SIMD optimization research for wc-rs text processing*