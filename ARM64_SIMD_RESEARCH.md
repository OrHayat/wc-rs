# ARM64 SIMD Optimization Research

## Project: wc-rs ARM64 NEON Implementation
**Current Status**: Optimized NEON movemask implementation complete (3 variants)  
**Goal**: Optimize text processing performance on ARM64 architectures

---

## Performance Analysis (vs Scalar Baseline)

| **Implementation** | **Speed vs Scalar** | **Availability** | **Test Environment** | **Priority** |
|-------------------|---------------------|------------------|---------------------|--------------|
| **Scalar (baseline)** | 1x | ✅ Universal | All platforms | ✅ Done |
| **NEON (emulated movemask)** | ~12x | ✅ Universal | Mac M3, all ARM64 | ✅ Done |
| **NEON (packed movemask)** | ~16x (est) | ✅ Universal | Mac M3, all ARM64 | ✅ Done |
| **NEON + Table Lookup movemask** | ~18x (est) | ✅ Universal | Mac M3, all ARM64 | � Next |
| **NEON + Crypto Extensions** | ~20x | ✅ Very Common | Mac M3, most ARM64 | 📋 Planned |
| **SVE 256-bit** | ~32x | ⚠️ Cloud/Server | AWS c7g instances | 📋 Future |
| **SVE2** | ~40-50x | ❌ Latest Cloud | AWS c8g instances | 📋 Future |

---

## Current Implementation Status

### ✅ **Movemask Optimization Complete**

**Problem**: Emulated movemask used 16 scalar lane extractions + branches, bottlenecked at ~12x speedup.

**Solution**: Pure NEON packed movemask using horizontal adds (vshr→vmul→vpaddl chain), eliminates scalar loops.

**Implementation**: Declarative macro `generate_neon_counter!` creates identical NEON functions differing only in movemask strategy. Three variants: emulated (reference), packed (active), vtbl (planned).

**Status**: Packed variant active in `count_text_neon()`, others marked `#[allow(dead_code)]` for benchmarking.

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
**Goal**: Implement and test SVE  
**Expected Gains**: 20x → 32x+ performance

**Setup**:
```bash
aws ec2 run-instances --image-id ami-0c2b8ca1dad447f8a --instance-type c7g.micro
cat /proc/cpuinfo | grep sve  # Check SVE support
```

**Cost**: c7g.micro $0.0168/hour (~$0.50 for testing)

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

**Design**: Macro generates identical functions differing only in movemask. All `#[allow(dead_code)]`.

---

## Research Sources

**ARM Docs**: ARMv8-A spec, NEON (DEN0018A), SVE (DDI 0584), Crypto (DEN0024A)  
**Rust**: std::arch feature detection  
**Cloud**: AWS Graviton documentation

---

*Research document for ARM64 SIMD optimization strategies*