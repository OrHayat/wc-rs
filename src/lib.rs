// Library interface for wc-rs
// Exposes the core counting functionality for use by fuzz targets and external crates

#[cfg(target_arch = "aarch64")]
mod wc_arm64;
pub mod wc_default;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod wc_x86;

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

/// SIMD implementation path used for counting
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CountingBackend {
    /// AVX-512 with byte operations (x86_64)
    Avx512,
    /// AVX2 256-bit vectors (x86_64)
    Avx2,
    /// SSE2 128-bit vectors (x86_64)
    Sse2,
    /// ARM SVE scalable vectors (aarch64)
    Sve,
    /// ARM NEON 128-bit vectors (aarch64)
    Neon,
    /// Scalar fallback implementation
    Scalar,
}

impl CountingBackend {
    /// Count text statistics using this SIMD path
    pub fn count_text(&self, content: &[u8], locale: LocaleEncoding) -> FileCounts {
        match self {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            CountingBackend::Avx512 => unsafe { wc_x86::count_text_avx512(content, locale) },
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            CountingBackend::Avx2 => unsafe { wc_x86::count_text_avx2(content, locale) },
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            CountingBackend::Sse2 => unsafe { wc_x86::count_text_sse2(content, locale) },
            #[cfg(all(target_arch = "aarch64", sve_available))]
            CountingBackend::Sve => unsafe { wc_arm64::count_text_sve(content, locale) },
            #[cfg(target_arch = "aarch64")]
            CountingBackend::Neon => unsafe { wc_arm64::count_text_neon(content, locale) },
            _ => wc_default::word_count_scalar(content, locale),
        }
    }
}
