#![no_main]

use libfuzzer_sys::fuzz_target;
use wc_rs::{CountingBackend, LocaleEncoding};

fuzz_target!(|data: &[u8]| {
    // Test that all available SIMD implementations produce the same result
    // This catches bugs where SIMD optimizations don't match the scalar code

    // Get scalar result as the reference
    let scalar_utf8 = CountingBackend::Scalar.count_text(data, LocaleEncoding::Utf8);
    let scalar_single = CountingBackend::Scalar.count_text(data, LocaleEncoding::SingleByte);

    // Test x86 SIMD paths if available
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse2") {
            let sse2_utf8 = CountingBackend::Sse2.count_text(data, LocaleEncoding::Utf8);
            let sse2_single = CountingBackend::Sse2.count_text(data, LocaleEncoding::SingleByte);
            assert_eq!(scalar_utf8, sse2_utf8, "SSE2 UTF-8 mismatch");
            assert_eq!(scalar_single, sse2_single, "SSE2 SingleByte mismatch");
        }

        if is_x86_feature_detected!("avx2") {
            let avx2_utf8 = CountingBackend::Avx2.count_text(data, LocaleEncoding::Utf8);
            let avx2_single = CountingBackend::Avx2.count_text(data, LocaleEncoding::SingleByte);
            assert_eq!(scalar_utf8, avx2_utf8, "AVX2 UTF-8 mismatch");
            assert_eq!(scalar_single, avx2_single, "AVX2 SingleByte mismatch");
        }

        if is_x86_feature_detected!("avx512bw") {
            let avx512_utf8 = CountingBackend::Avx512.count_text(data, LocaleEncoding::Utf8);
            let avx512_single = CountingBackend::Avx512.count_text(data, LocaleEncoding::SingleByte);
            assert_eq!(scalar_utf8, avx512_utf8, "AVX512 UTF-8 mismatch");
            assert_eq!(scalar_single, avx512_single, "AVX512 SingleByte mismatch");
        }
    }

    // Test ARM SIMD paths if available
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            let neon_utf8 = CountingBackend::Neon.count_text(data, LocaleEncoding::Utf8);
            let neon_single = CountingBackend::Neon.count_text(data, LocaleEncoding::SingleByte);
            assert_eq!(scalar_utf8, neon_utf8, "NEON UTF-8 mismatch");
            assert_eq!(scalar_single, neon_single, "NEON SingleByte mismatch");
        }

        if std::arch::is_aarch64_feature_detected!("sve") {
            let sve_utf8 = CountingBackend::Sve.count_text(data, LocaleEncoding::Utf8);
            let sve_single = CountingBackend::Sve.count_text(data, LocaleEncoding::SingleByte);
            assert_eq!(scalar_utf8, sve_utf8, "SVE UTF-8 mismatch");
            assert_eq!(scalar_single, sve_single, "SVE SingleByte mismatch");
        }
    }
});
