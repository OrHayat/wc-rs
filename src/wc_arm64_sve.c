// ARM SVE implementation for wc-rs
// Ports NEON logic to SVE for scalable vector processing

// Require SVE support at compile time
#ifndef __ARM_FEATURE_SVE
#error "SVE intrinsics not available. Compile with -march=armv8.2-a+sve (or later)"
#endif

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <stdio.h>
#include <arm_sve.h>

// Platform-specific headers for CPU detection
#if defined(__linux__)
#include <sys/auxv.h>
#elif defined(__APPLE__)
#include <sys/sysctl.h>
#endif

// Match Rust's FileCounts structure
typedef struct {
    size_t lines;
    size_t words;
    size_t bytes;
    size_t chars;
} FileCounts;

// Result struct for checked version with success flag
typedef struct {
    FileCounts counts;
    bool success;  // true if SVE was available, false otherwise
} FileCountsResult;

// Match Rust's LocaleEncoding enum
enum LocaleEncoding {
    LOCALE_C = 0,
    LOCALE_UTF8 = 1
};

// ============================================================================
// Forward Declarations
// ============================================================================

FileCounts count_text_sve_c_unchecked(
    const uint8_t* content,
    size_t len,
    enum LocaleEncoding locale
);

FileCountsResult count_text_sve_c_checked(
    const uint8_t* content,
    size_t len,
    enum LocaleEncoding locale
);

// ============================================================================
// CPU Feature Detection
// ============================================================================

// Check if CPU supports SVE at runtime
static inline bool cpu_supports_sve(void) {
#if defined(__linux__)
    // Linux: use getauxval to check HWCAP
    #ifndef HWCAP_SVE
    #define HWCAP_SVE (1 << 22)
    #endif
    unsigned long hwcaps = getauxval(AT_HWCAP);
    return (hwcaps & HWCAP_SVE) != 0;

#elif defined(__APPLE__)
    // macOS: use sysctl
    int has_sve = 0;
    size_t size = sizeof(has_sve);

    if (sysctlbyname("hw.optional.arm.FEAT_SVE", &has_sve, &size, NULL, 0) == 0) {
        return has_sve == 1;
    }
    return false;

#else
    // Unknown platform - assume no SVE
    return false;
#endif
}

// ============================================================================
// SVE Helper Functions
// ============================================================================

// Count newlines in SVE vector
static inline size_t sve_count_newlines(svbool_t pg, svuint8_t chunk) {
    svuint8_t newline_vec = svdup_n_u8('\n');
    svbool_t newline_cmp = svcmpeq_u8(pg, chunk, newline_vec);
    return svcntp_b8(pg, newline_cmp);
}

// Check if vector contains non-ASCII bytes (>= 0x80)
static inline bool sve_has_non_ascii(svbool_t pg, svuint8_t chunk) {
    svuint8_t ascii_threshold = svdup_n_u8(0x80);
    svbool_t has_non_ascii_mask = svcmpge_u8(pg, chunk, ascii_threshold);
    return svcntp_b8(pg, has_non_ascii_mask) > 0;
}

// Count UTF-8 characters (non-continuation bytes)
// Continuation bytes match pattern 10xxxxxx (0b10000000)
static inline size_t sve_count_utf8_chars(svbool_t pg, svuint8_t chunk) {
    svuint8_t cont_mask = svdup_n_u8(0b11000000);
    svuint8_t cont_pattern = svdup_n_u8(0b10000000);

    svuint8_t masked = svand_u8_z(pg, chunk, cont_mask);
    svbool_t is_continuation = svcmpeq_u8(pg, masked, cont_pattern);
    svbool_t is_not_continuation = svnot_b_z(pg, is_continuation);

    return svcntp_b8(pg, is_not_continuation);
}

// Detect ASCII whitespace: space (0x20) or range [0x09-0x0D]
// Returns count of whitespace bytes for tracking word boundaries
static inline size_t sve_count_whitespace(svbool_t pg, svuint8_t chunk, bool *all_ws, bool *any_ws) {
    svuint8_t ws_min = svdup_n_u8(0x09);  // tab
    svuint8_t ws_max = svdup_n_u8(0x0D);  // carriage return
    svuint8_t space = svdup_n_u8(0x20);

    // Range check: [0x09, 0x0D]
    svbool_t in_range = svand_b_z(pg,
                                   svcmpge_u8(pg, chunk, ws_min),
                                   svcmple_u8(pg, chunk, ws_max));

    // Check space
    svbool_t is_space = svcmpeq_u8(pg, chunk, space);

    // Combine: is whitespace if in_range OR is_space
    svbool_t is_ws = svorr_b_z(pg, in_range, is_space);

    size_t ws_count = svcntp_b8(pg, is_ws);
    size_t total_count = svcntp_b8(pg, pg);

    *all_ws = (ws_count == total_count);
    *any_ws = (ws_count > 0);

    return ws_count;
}

// Count word starts in SVE vector
// A word start is: current byte is not whitespace AND previous byte was whitespace
// This is a simplified version - extract to array and process
static inline size_t sve_count_words(svbool_t pg, svuint8_t chunk, bool *seen_space, bool *last_is_ws) {
    svuint8_t ws_min = svdup_n_u8(0x09);
    svuint8_t ws_max = svdup_n_u8(0x0D);
    svuint8_t space = svdup_n_u8(0x20);

    // Detect whitespace
    svbool_t in_range = svand_b_z(pg,
                                   svcmpge_u8(pg, chunk, ws_min),
                                   svcmple_u8(pg, chunk, ws_max));
    svbool_t is_space = svcmpeq_u8(pg, chunk, space);
    svbool_t is_ws = svorr_b_z(pg, in_range, is_space);

    // Store whitespace mask to array for processing
    // SVE vector length ranges from 128 to 2048 bits (in 128-bit increments)
    // Maximum: 2048 bits = 256 bytes
    // Reference: https://developer.arm.com/documentation/102476/0100/Introducing-SVE
    uint8_t ws_array[256];
    svuint8_t ones_vec = svdup_n_u8(1);
    svuint8_t ws_mask_vec = svsel_u8(is_ws, ones_vec, svdup_n_u8(0));
    svst1_u8(pg, ws_array, ws_mask_vec);

    // Count word starts using scalar logic
    size_t active_count = svcntp_b8(pg, pg);
    size_t word_count = 0;
    bool prev_was_ws = *seen_space;

    for (size_t i = 0; i < active_count; i++) {
        bool is_ws_byte = (ws_array[i] != 0);
        bool is_not_ws = !is_ws_byte;

        // Word start: not whitespace AND previous was whitespace
        if (is_not_ws && prev_was_ws) {
            word_count++;
        }

        prev_was_ws = is_ws_byte;
    }

    // Update state
    *last_is_ws = prev_was_ws;
    *seen_space = *last_is_ws;

    return word_count;
}

// ============================================================================
// Public API Functions
// ============================================================================


// Checked version: verifies CPU supports SVE at runtime (safe)
// Returns FileCountsResult with success flag
FileCountsResult count_text_sve_c_checked(
    const uint8_t* content,
    size_t len,
    enum LocaleEncoding locale
) {
    FileCountsResult result={0};

    // Check if CPU supports SVE
    if (!cpu_supports_sve()) {
        // CPU doesn't support SVE - return failure
        result.counts = (FileCounts){
            .lines = 0,
            .words = 0,
            .bytes = 0,
            .chars = 0
        };
        result.success = false;
        return result;
    }

    // CPU supports SVE - call unchecked version
    result.counts = count_text_sve_c_unchecked(content, len, locale);
    result.success = true;
    return result;
}

// Unchecked version: assumes SVE is available (fast, no runtime check)
// Caller MUST verify CPU supports SVE before calling this!
FileCounts count_text_sve_c_unchecked(
    const uint8_t* content,
    size_t len,
    enum LocaleEncoding locale
) {
    FileCounts result = {0};

    // Initialize counts
    result.lines = 0;
    result.words = 0;
    result.chars = 0;
    result.bytes = len;

    if (len == 0) {
        return result;
    }

    // Get SVE vector length in bytes
    size_t vl = svcntb();

    // Word counting state
    bool seen_space = true;
    bool last_is_ws = true;

    size_t i = 0;

    // Process full vectors
    while (i + vl <= len) {
        // Create predicate for full vector
        svbool_t pg = svptrue_b8();

        // Load vector
        svuint8_t chunk = svld1_u8(pg, content + i);

        // Check for non-ASCII (for UTF-8 handling)
        bool has_non_ascii = sve_has_non_ascii(pg, chunk);

        // For simplicity, use SIMD path for ASCII or C locale
        // Production code would fall back to scalar for complex UTF-8
        if (!has_non_ascii || locale == LOCALE_C) {
            // Count newlines
            result.lines += sve_count_newlines(pg, chunk);

            // Count characters
            if (locale == LOCALE_UTF8) {
                result.chars += sve_count_utf8_chars(pg, chunk);
            } else {
                result.chars += vl;
            }

            // Count words
            result.words += sve_count_words(pg, chunk, &seen_space, &last_is_ws);
        } else {
            // Fallback: scalar processing for non-ASCII UTF-8
            // This is simplified - production code would use carry buffer
            for (size_t j = 0; j < vl; j++) {
                uint8_t byte = content[i + j];

                if (byte == '\n') {
                    result.lines++;
                }

                // UTF-8 character counting: skip continuation bytes
                if ((byte & 0b11000000) != 0b10000000) {
                    result.chars++;
                }

                // Word counting
                bool is_ws = (byte == ' ' || (byte >= 0x09 && byte <= 0x0D));
                if (!is_ws && seen_space) {
                    result.words++;
                }
                seen_space = is_ws;
            }
        }

        i += vl;
    }

    // Process remainder with partial vector
    if (i < len) {
        size_t remaining = len - i;
        svbool_t pg = svwhilelt_b8_u64(0, remaining);

        // Load partial vector
        svuint8_t chunk = svld1_u8(pg, content + i);

        bool has_non_ascii = sve_has_non_ascii(pg, chunk);

        if (!has_non_ascii || locale == LOCALE_C) {
            result.lines += sve_count_newlines(pg, chunk);

            if (locale == LOCALE_UTF8) {
                result.chars += sve_count_utf8_chars(pg, chunk);
            } else {
                result.chars += remaining;
            }

            result.words += sve_count_words(pg, chunk, &seen_space, &last_is_ws);
        } else {
            // Scalar fallback for remainder
            for (size_t j = 0; j < remaining; j++) {
                uint8_t byte = content[i + j];

                if (byte == '\n') {
                    result.lines++;
                }

                if ((byte & 0b11000000) != 0b10000000) {
                    result.chars++;
                }

                bool is_ws = (byte == ' ' || (byte >= 0x09 && byte <= 0x0D));
                if (!is_ws && seen_space) {
                    result.words++;
                }
                seen_space = is_ws;
            }
        }
    }

    return result;
}