// Library interface for wc-rs
// Exposes the core counting functionality for use by fuzz targets and external crates

#[cfg(target_arch = "aarch64")]
mod wc_arm64;
#[cfg(all(test, target_arch = "aarch64"))]
mod wc_arm64_test;
pub mod wc_default;
#[cfg(test)]
mod wc_default_test;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod wc_x86;
#[cfg(all(test, any(target_arch = "x86", target_arch = "x86_64")))]
mod wc_x86_test;

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

// Private marker to prevent external construction of CountingBackend variants
// NOT pub - this is intentionally private!
#[derive(Debug, Clone, Copy, PartialEq)]
struct Private;

/// SIMD implementation path used for counting.
///
/// **IMPORTANT**: Variants cannot be constructed directly from outside this crate
/// due to the private `Private` field. Use `CountingBackend::detect()` to safely
/// obtain a backend supported by the current CPU.
#[allow(private_interfaces)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CountingBackend {
    /// AVX-512 with byte operations (x86_64)
    Avx512(Private),
    /// AVX2 256-bit vectors (x86_64)
    Avx2(Private),
    /// SSE2 128-bit vectors (x86_64)
    Sse2(Private),
    /// ARM SVE scalable vectors (aarch64)
    Sve(Private),
    /// ARM NEON 128-bit vectors (aarch64)
    Neon(Private),
    /// Scalar fallback implementation
    Scalar(Private),
}

impl std::fmt::Display for CountingBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CountingBackend::Avx512(_) => write!(f, "AVX-512"),
            CountingBackend::Avx2(_) => write!(f, "AVX2"),
            CountingBackend::Sse2(_) => write!(f, "SSE2"),
            CountingBackend::Sve(_) => write!(f, "SVE"),
            CountingBackend::Neon(_) => write!(f, "NEON"),
            CountingBackend::Scalar(_) => write!(f, "Scalar"),
        }
    }
}

impl CountingBackend {
    /// Detect which SIMD backend is supported by the current CPU at runtime.
    ///
    /// This performs runtime feature detection and returns the fastest available
    /// SIMD implementation. Use this to safely select a backend before calling
    /// `count_text()`.
    ///
    /// # Example
    /// ```
    /// use wc_rs::{CountingBackend, LocaleEncoding};
    ///
    /// // Detect the best SIMD backend for this CPU
    /// let backend = CountingBackend::detect();
    /// println!("Using backend: {}", backend);
    ///
    /// // Count statistics in UTF-8 mode
    /// let result = backend.count_text(b"hello world\n", LocaleEncoding::Utf8);
    /// assert_eq!(result.lines, 1);
    /// assert_eq!(result.words, 2);
    /// assert_eq!(result.bytes, 12);
    /// assert_eq!(result.chars, 12);
    ///
    /// // Count in single-byte mode (ASCII-only whitespace)
    /// let result = backend.count_text(b"foo bar", LocaleEncoding::SingleByte);
    /// assert_eq!(result.lines, 0);
    /// assert_eq!(result.words, 2);
    /// assert_eq!(result.bytes, 7);
    /// assert_eq!(result.chars, 7);
    /// ```
    pub fn detect() -> Self {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx512bw") {
                return CountingBackend::Avx512(Private);
            } else if is_x86_feature_detected!("avx2") {
                return CountingBackend::Avx2(Private);
            } else if is_x86_feature_detected!("sse2") {
                return CountingBackend::Sse2(Private);
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            #[cfg(sve_available)]
            if std::arch::is_aarch64_feature_detected!("sve") {
                return CountingBackend::Sve(Private);
            }

            if std::arch::is_aarch64_feature_detected!("neon") {
                return CountingBackend::Neon(Private);
            }
        }

        CountingBackend::Scalar(Private)
    }

    /// Construct a Scalar backend.
    ///
    /// # Safety
    ///
    /// This bypasses the normal safety checks that prevent construction
    /// of backends without CPU feature detection.
    ///
    /// Scalar backend is safe on all CPUs.
    ///
    /// # Example
    /// ```ignore
    /// unsafe {
    ///     let backend = CountingBackend::new_scalar_unchecked();
    ///     let result = backend.count_text(b"test", LocaleEncoding::Utf8);
    /// }
    /// ```
    pub unsafe fn new_scalar_unchecked() -> Self {
        CountingBackend::Scalar(Private)
    }

    /// Construct specific backends for consistency testing.
    ///
    /// # Safety
    ///
    /// **DANGER**: Caller MUST verify the CPU supports the requested feature
    /// BEFORE calling `count_text()`, or the program will crash with SIGILL
    /// (illegal instruction).
    ///
    /// Always guard usage with CPU feature detection:
    ///
    /// # Example
    /// ```ignore
    /// #[cfg(target_arch = "x86_64")]
    /// if is_x86_feature_detected!("avx2") {
    ///     unsafe {
    ///         let backend = CountingBackend::new_unchecked("avx2").unwrap();
    ///         // Safe to use now - we verified AVX2 support
    ///         let result = backend.count_text(data, LocaleEncoding::Utf8);
    ///     }
    /// }
    /// ```
    ///
    /// # Parameters
    /// - `backend_type`: One of: "scalar", "sse2", "avx2", "avx512", "neon", "sve"
    ///
    /// # Returns
    /// - `Some(backend)` if the backend name is valid
    /// - `None` if the backend name is invalid
    pub unsafe fn new_unchecked(backend_type: &str) -> Option<Self> {
        match backend_type {
            "scalar" => Some(CountingBackend::Scalar(Private)),
            "sse2" => Some(CountingBackend::Sse2(Private)),
            "avx2" => Some(CountingBackend::Avx2(Private)),
            "avx512" => Some(CountingBackend::Avx512(Private)),
            "neon" => Some(CountingBackend::Neon(Private)),
            "sve" => Some(CountingBackend::Sve(Private)),
            _ => None,
        }
    }

    /// Count text statistics using this SIMD path.
    ///
    /// # Safety
    ///
    /// **NOTE**: Backends can only be obtained through `CountingBackend::detect()`,
    /// which ensures the backend is supported by the current CPU. Direct construction
    /// of backend variants is prevented to avoid crashes from unsupported SIMD instructions.
    ///
    /// # Example
    /// ```
    /// use wc_rs::{CountingBackend, LocaleEncoding};
    ///
    /// // Safely detect and use the best available backend
    /// let backend = CountingBackend::detect();
    /// let result = backend.count_text(b"hello world", LocaleEncoding::Utf8);
    /// assert_eq!(result.words, 2);
    /// ```
    pub fn count_text(&self, content: &[u8], locale: LocaleEncoding) -> FileCounts {
        match self {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            CountingBackend::Avx512(_) => unsafe { wc_x86::count_text_avx512(content, locale) },
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            CountingBackend::Avx2(_) => unsafe { wc_x86::count_text_avx2(content, locale) },
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            CountingBackend::Sse2(_) => unsafe { wc_x86::count_text_sse2(content, locale) },
            #[cfg(all(target_arch = "aarch64", sve_available))]
            CountingBackend::Sve(_) => unsafe { wc_arm64::count_text_sve(content, locale) },
            #[cfg(target_arch = "aarch64")]
            CountingBackend::Neon(_) => unsafe { wc_arm64::count_text_neon(content, locale) },
            _ => wc_default::word_count_scalar(content, locale),
        }
    }
}
