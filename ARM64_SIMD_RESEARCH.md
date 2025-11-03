# ARM64 SIMD Optimization Research

## Project: wc-rs ARM64 NEON Implementation
**Current Status**: Basic NEON implementation complete  
**Goal**: Optimize text processing performance on ARM64 architectures

---

## Performance Analysis (vs Scalar Baseline)

| **Implementation** | **Speed vs Scalar** | **Availability** | **Test Environment** | **Priority** |
|-------------------|---------------------|------------------|---------------------|--------------|
| **Scalar (baseline)** | 1x | ✅ Universal | All platforms | ✅ Done |
| **Current NEON (unoptimized)** | ~12x | ✅ Universal | Mac M3, all ARM64 | ✅ Done |
| **Optimized NEON movemask** | ~16x | ✅ Universal | Mac M3, all ARM64 | 🔄 Next |
| **NEON + Table Lookup** | ~18x | ✅ Universal | Mac M3, all ARM64 | 📋 Planned |
| **NEON + Crypto Extensions** | ~20x | ✅ Very Common | Mac M3, most ARM64 | 📋 Planned |
| **SVE 256-bit** | ~32x | ⚠️ Cloud/Server | AWS c7g instances | 📋 Future |
| **SVE2** | ~40-50x | ❌ Latest Cloud | AWS c8g instances | 📋 Future |

---

## Current Implementation Issues

### **Movemask Bottleneck**
The current `neon_movemask_u8x8` function is severely limiting performance:

```rust
// CURRENT: Slow scalar extraction (kills SIMD benefits)
for i in 0..8 {
    let byte = vget_lane_u8(vec, i);  // 8 individual extractions
    if (byte & 0x80) != 0 {           // 8 scalar conditions
        mask |= 1u64 << i;
    }
}
```

**Problems:**
- 16 scalar operations per 16-byte vector
- Destroys SIMD parallelism
- Reduces theoretical 16x speedup to ~12x

---

## Optimization Strategies

### **1. Optimized NEON Movemask** 
**Target**: 16x speedup (vs current 12x)  
**Compatibility**: ✅ Universal ARM64  
**Test Environment**: Mac M3

**Approach**: Replace scalar loop with pure NEON operations
```rust
// PROPOSED: Pure NEON bit extraction
let shifted = vshrq_n_u8(vec, 7);           // Extract high bits in parallel
let narrowed = vmovn_u16(vreinterpretq_u16_u8(shifted)); // Pack bits
// Additional NEON bit manipulation...
```

### **2. NEON Table Lookup Optimization**
**Target**: 18x speedup  
**Compatibility**: ✅ Universal ARM64  
**Test Environment**: Mac M3

**Approach**: Use `vtbl` instruction for whitespace detection
```rust
// PROPOSED: Table-based whitespace detection
let whitespace_table = [/* lookup table for ASCII whitespace */];
let whitespace_mask = vtbl1_u8(table, chunk);
```

### **3. Crypto Extensions Integration**
**Target**: 20x speedup  
**Compatibility**: ✅ Very Common (Apple Silicon, most modern ARM64)  
**Test Environment**: Mac M3
**Detection**: `std::arch::is_aarch64_feature_detected!("aes")`

**Approach**: Leverage crypto instructions for faster bit manipulation
```rust
// PROPOSED: Use AES/SHA instructions for parallel operations
if std::arch::is_aarch64_feature_detected!("aes") {
    // Advanced bit manipulation using crypto instructions
}
```

### **4. SVE (Scalable Vector Extensions)**
**Target**: 32x speedup (256-bit vectors)  
**Compatibility**: ⚠️ Cloud/Server only  
**Test Environment**: AWS c7g instances  
**Detection**: `std::arch::is_aarch64_feature_detected!("sve")`

**Approach**: Variable-length vectors (128-2048 bits)
```rust
// PROPOSED: SVE implementation
if std::arch::is_aarch64_feature_detected!("sve") {
    // Process with variable vector lengths
    // Automatically adapts to CPU vector width
}
```

### **5. SVE2 (Enhanced SVE)**
**Target**: 40-50x speedup  
**Compatibility**: ❌ Very Limited (AWS c8g instances)  
**Test Environment**: AWS c8g instances  
**Detection**: `std::arch::is_aarch64_feature_detected!("sve2")`

---

## Hardware Compatibility Matrix

### **Consumer Devices**
| **Device** | **NEON** | **Crypto** | **SVE** | **SVE2** | **Notes** |
|------------|----------|------------|---------|----------|-----------|
| Apple M1/M2/M3/M4 | ✅ | ✅ | ❌ | ❌ | Perfect for NEON+Crypto testing |
| iPhone (A7+) | ✅ | ✅ | ❌ | ❌ | Since iPhone 5S (2013) |
| iPad (A7+) | ✅ | ✅ | ❌ | ❌ | Since iPad Air (2013) |
| Qualcomm 8cx/X Elite | ✅ | ✅ | ❌ | ❌ | Windows on ARM laptops |
| Samsung Exynos | ✅ | ✅ | ❌ | ❌ | Most Android phones |

### **Cloud/Server Instances**
| **Instance Type** | **NEON** | **Crypto** | **SVE** | **SVE2** | **Cost** |
|-------------------|----------|------------|---------|----------|----------|
| AWS c6g (Graviton2) | ✅ | ✅ | ❌ | ❌ | $0.034/hr |
| AWS c7g (Graviton3) | ✅ | ✅ | ✅ | ❌ | $0.036/hr |
| AWS c8g (Graviton4) | ✅ | ✅ | ✅ | ✅ | $0.038/hr |
| Oracle Ampere Altra | ✅ | ✅ | ⚠️ | ❌ | Variable |

### **Specialized Hardware**
| **System** | **NEON** | **Crypto** | **SVE** | **SVE2** | **Vector Width** |
|------------|----------|------------|---------|----------|------------------|
| Fujitsu A64FX | ✅ | ✅ | ✅ | ❌ | 512-bit |
| ARM Neoverse V1 | ✅ | ✅ | ✅ | ❌ | 256-bit |
| ARM Neoverse V2 | ✅ | ✅ | ✅ | ✅ | 256-bit |

---

## Testing Strategy

### **Local Development (Mac M3)**
**Goal**: Optimize NEON and add Crypto extensions  
**Expected Gains**: 12x → 20x performance  

**Tasks**:
1. ✅ Implement basic NEON (done)
2. 🔄 Optimize movemask implementation  
3. 📋 Add table lookup optimization
4. 📋 Integrate crypto extensions
5. 📋 Benchmark against current implementation

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

### **Current Structure**
```
wc_amd64.rs
├── count_text_simd()           // Entry point with feature detection
├── count_text_neon()           // Basic NEON implementation  
├── neon_movemask_u8()          // BOTTLENECK: Slow bit extraction
└── count_text_scalar()         // Fallback implementation
```

### **Proposed Structure**
```
wc_amd64.rs
├── count_text_simd()           // Enhanced feature detection
├── count_text_sve2()           // SVE2 implementation
├── count_text_sve()            // SVE implementation
├── count_text_neon_crypto()    // NEON + Crypto extensions
├── count_text_neon_optimized() // Optimized NEON
└── count_text_scalar()         // Fallback implementation
```

### **Feature Detection**
```rust
pub fn count_text_simd(content: &[u8]) -> Option<FileCounts> {
    if std::arch::is_aarch64_feature_detected!("sve2") {
        // Use SVE2 (~50x speedup)
    } else if std::arch::is_aarch64_feature_detected!("sve") {
        // Use SVE (~32x speedup)  
    } else if std::arch::is_aarch64_feature_detected!("aes") {
        // Use NEON + Crypto (~20x speedup)
    } else if std::arch::is_aarch64_feature_detected!("neon") {
        // Use optimized NEON (~16x speedup)
    } else {
        None
    }
}
```

---

## Research Sources

### **Official Documentation**
- **ARM Architecture Reference Manual**: ARMv8-A specification
- **ARM Developer Documentation**: developer.arm.com
- **Apple Developer Documentation**: developer.apple.com/documentation/kernel
- **AWS Graviton Documentation**: aws.amazon.com/ec2/graviton/

### **Feature Detection References**
- **Rust std::arch**: doc.rust-lang.org/std/arch/
- **Linux /proc/cpuinfo**: Documentation/admin-guide/cputopology.rst
- **macOS sysctl**: man 3 sysctlbyname

### **Performance Research**
- **ARM NEON Programming Guide**: ARM DEN0018A
- **SVE Programming Guide**: ARM DDI 0584
- **Crypto Extensions Guide**: ARM DEN0024A

---

## Next Steps

1. 🔄 Implement optimized NEON movemask
2. 📋 Document current NEON implementation (analysis/reference)
3. 📋 Test optimized version on Mac M3
4. 📋 Measure performance improvements
5. 📋 Add crypto extensions support
6. 📋 Implement table lookup optimization
7. 📋 Create comprehensive test suite
8. 📋 Set up AWS testing environment
9. 📋 Implement SVE support
10. 📋 Benchmark on c7g instances
11. 📋 Add benchmarking infrastructure
12. 📋 Add SVE2 support for c8g instances

---

## Questions for Future Research

1. **Memory Bandwidth**: Does SVE's wider vectors saturate memory bandwidth?
2. **Real-world Files**: How do optimizations perform on different text types?
3. **Power Consumption**: Energy efficiency comparison between implementations?
4. **Cache Effects**: Impact of larger vector operations on cache performance?
5. **Compilation**: Does Rust's LLVM backend optimize our SIMD code effectively?

---

*Research document for ARM64 SIMD optimization strategies*