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
#include "../vendor/utf8.h"

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
// Unicode Whitespace Detection
// ============================================================================

// Forward declaration
static inline bool is_unicode_whitespace(utf8_int32_t codepoint);

// ============================================================================
// UTF-8 Boundary Detection
// ============================================================================

// Detect incomplete UTF-8 sequence at the end of a buffer.
// Returns the number of bytes (0-3) that form an incomplete sequence.
static inline size_t detect_incomplete_utf8_suffix(const uint8_t* data, size_t len) {
    if (len == 0) return 0;

    // Scan backwards from end looking for UTF-8 start byte
    for (size_t i = 0; i < 4 && i < len; i++) {
        size_t pos = len - 1 - i;
        uint8_t byte = data[pos];
        size_t bytes_from_here = len - pos;

        // ASCII (0xxxxxxx)
        if ((byte & 0x80) == 0) {
            return 0; // Complete
        }
        // 2-byte start (110xxxxx)
        else if ((byte & 0xE0) == 0xC0) {
            return (bytes_from_here < 2) ? bytes_from_here : 0;
        }
        // 3-byte start (1110xxxx)
        else if ((byte & 0xF0) == 0xE0) {
            return (bytes_from_here < 3) ? bytes_from_here : 0;
        }
        // 4-byte start (11110xxx)
        else if ((byte & 0xF8) == 0xF0) {
            return (bytes_from_here < 4) ? bytes_from_here : 0;
        }
        // Continuation byte (10xxxxxx) - keep looking
        else if ((byte & 0xC0) == 0x80) {
            continue;
        }
        // Invalid - treat as complete
        else {
            return 0;
        }
    }

    // All continuation bytes - incomplete
    return (len > 4) ? 4 : len;
}

// Process UTF-8 data with carry buffer for incomplete sequences
// Returns new seen_space state
static bool process_utf8_with_carry(
    const uint8_t* data,
    size_t len,
    uint8_t* carry,
    size_t* carry_len,
    FileCounts* result,
    bool seen_space
) {
    // Combine carry + new data into working buffer
    uint8_t buffer[260]; // Max: 3 carry + 256 chunk + 1 safety
    size_t buffer_len = 0;

    // Copy carry bytes
    for (size_t i = 0; i < *carry_len; i++) {
        buffer[buffer_len++] = carry[i];
    }

    // Copy new data
    for (size_t i = 0; i < len && buffer_len < sizeof(buffer); i++) {
        buffer[buffer_len++] = data[i];
    }

    // Detect incomplete UTF-8 at end
    size_t incomplete = detect_incomplete_utf8_suffix(buffer, buffer_len);
    size_t process_len = buffer_len - incomplete;

    // Process complete characters
    void* ptr = (void*)buffer;
    void* end = (void*)(buffer + process_len);

    while (ptr < end) {
        utf8_int32_t codepoint;
        void* next_ptr = utf8codepoint(ptr, &codepoint);

        if (next_ptr == ptr || next_ptr > end) {
            // Invalid UTF-8 - skip byte
            ptr = (uint8_t*)ptr + 1;
            continue;
        }

        result->chars++;

        if (codepoint == '\n') {
            result->lines++;
        }

        bool is_ws = is_unicode_whitespace(codepoint);
        if (!is_ws && seen_space) {
            result->words++;
        }
        seen_space = is_ws;

        ptr = next_ptr;
    }

    // Save incomplete bytes to carry
    *carry_len = incomplete;
    for (size_t i = 0; i < incomplete; i++) {
        carry[i] = buffer[process_len + i];
    }

    return seen_space;
}

// Check if a Unicode codepoint is whitespace
// Matches Rust's char::is_whitespace() behavior
static inline bool is_unicode_whitespace(utf8_int32_t codepoint) {
    // ASCII whitespace (fast path)
    if (codepoint == 0x20 || (codepoint >= 0x09 && codepoint <= 0x0D)) {
        return true;
    }

    // Unicode whitespace characters
    switch (codepoint) {
        case 0x0085: // Next Line (NEL)
        case 0x00A0: // No-Break Space (NBSP)
        case 0x1680: // Ogham Space Mark
        case 0x2000: case 0x2001: case 0x2002: case 0x2003: case 0x2004:
        case 0x2005: case 0x2006: case 0x2007: case 0x2008: case 0x2009:
        case 0x200A: // Various spaces
        case 0x2028: // Line Separator
        case 0x2029: // Paragraph Separator
        case 0x202F: // Narrow No-Break Space
        case 0x205F: // Medium Mathematical Space
        case 0x3000: // Ideographic Space
            return true;
        default:
            return false;
    }
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

    // UTF-8 carry buffer for incomplete sequences at chunk boundaries
    uint8_t carry[4] = {0};
    size_t carry_len = 0;

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
            // Fallback: scalar processing for non-ASCII UTF-8 with carry buffer
            seen_space = process_utf8_with_carry(
                content + i, vl, carry, &carry_len, &result, seen_space
            );
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
            // Scalar fallback for remainder with carry buffer
            seen_space = process_utf8_with_carry(
                content + i, remaining, carry, &carry_len, &result, seen_space
            );
        }
    }

    // Flush any remaining carry (incomplete UTF-8 at end of input)
    if (carry_len > 0) {
        // Process carry with empty new data to flush
        process_utf8_with_carry(NULL, 0, carry, &carry_len, &result, seen_space);
    }

    return result;
}