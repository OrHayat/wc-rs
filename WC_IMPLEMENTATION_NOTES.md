# WC Implementation Notes

## Session Summary: Understanding `wc` Behavior and Locale Handling

### The Problem We Investigated

When testing `wc` with different flag combinations, we discovered inconsistent output:

```bash
wc ./ARM64_SIMD_RESEARCH.md
# Output: 148  1062  8059

wc -lwm ./ARM64_SIMD_RESEARCH.md
# Output: 148  1060  7853
```

**Key differences:**
- Words: 1062 vs 1060 (2 fewer)
- Last column: 8059 vs 7853 (206 fewer)

### Root Cause: Byte Count vs Character Count

The difference is due to how `wc` counts:

- **Default `wc`** (no flags): Shows `lines words bytes`
- **`wc -lwm`**: Shows `lines words chars` (UTF-8 character count)

The file contains **206 multi-byte UTF-8 characters**, so:
- Bytes: 8059
- Characters: 7853
- Difference: 206 bytes from multi-byte characters

### Locale Dependency

The behavior changes based on locale settings:

```bash
# UTF-8 locale
wc -lwm file.txt          # 148  1060  7853

# C locale (ASCII only)
LC_ALL=C wc file.txt      # 148  1060  8059
LC_ALL=C wc -lwm file.txt # 148  1060  8059 (bytes = chars)
```

**Key insight:** `LC_CTYPE=C` treats every byte as a character, giving wrong counts for UTF-8 text.

## POSIX Standard Behavior

According to POSIX:

- **`-c` flag**: Count bytes
- **`-m` flag**: Count characters (locale-aware)
- **Default (no flags)**: Output `lines words bytes`
- **`-l`, `-w`**: Count lines and words

## Implementation Recommendations

### 1. Always Track Both Bytes and Characters

```rust
struct FileCounts {
    lines: usize,
    words: usize,
    bytes: usize,  // Always count
    chars: usize,  // Always count (UTF-8 aware)
}
```

### 2. UTF-8 Character Counting

**Simple and correct approach:**
Count all bytes that are NOT UTF-8 continuation bytes.

```rust
// UTF-8 continuation bytes match pattern 10xxxxxx (0x80-0xBF)
for byte in content {
    if (byte & 0b11000000) != 0b10000000 {
        char_count += 1;
    }
}
```

**Why this works:**
- ASCII: `0xxxxxxx` → counts as character ✓
- UTF-8 start: `110xxxxx`, `1110xxxx`, `11110xxx` → counts ✓
- UTF-8 continuation: `10xxxxxx` → doesn't count ✓

### 3. Locale Detection

```rust
use std::env;

fn is_utf8_locale() -> bool {
    let locale = env::var("LC_ALL")
        .or_else(|_| env::var("LC_CTYPE"))
        .or_else(|_| env::var("LANG"))
        .unwrap_or_default();

    locale.to_uppercase().contains("UTF-8") ||
    locale.to_uppercase().contains("UTF8")
}
```

### 4. Locale-Specific Behavior

#### LC_CTYPE=UTF-8 (UTF-8 locale)
```rust
fn count_utf8(content: &[u8]) -> FileCounts {
    // Use SIMD implementation
    // Count UTF-8 characters (skip continuation bytes)
    // Use char::is_whitespace() for word boundaries
}
```

#### LC_CTYPE=C (C locale)
```rust
fn count_c_locale(content: &[u8]) -> FileCounts {
    FileCounts {
        bytes: content.len(),
        chars: content.len(),  // bytes = chars in C locale
        lines: count_newlines(content),
        words: count_ascii_words(content),
    }
}
```

**Important:** In C locale, every byte is treated as a character. This gives **wrong results** for UTF-8 text, but matches GNU `wc` behavior.

### 5. Word Counting Strategy

**UTF-8 locale:**
- Use `char::is_whitespace()` for proper Unicode whitespace detection
- Track transitions from whitespace → non-whitespace

**C locale:**
- Check only ASCII whitespace: space, tab, newline, `\r`, `\x0B`, `\x0C`
- Byte-level processing

**Standard whitespace characters:**
```rust
fn is_ascii_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\n' | b'\t' | b'\r' | 0x0B | 0x0C)
}
```

## Why LC_CTYPE=C Breaks UTF-8

Example with Hebrew letter ש:
- UTF-8 bytes: `0xD7 0xA9` (2 bytes)
- In C locale: counts as **2 characters** (wrong!)
- In UTF-8 locale: counts as **1 character** (correct)

Example with checkmark ✅:
- UTF-8 bytes: `0xE2 0x9C 0x85` (3 bytes)
- In C locale: counts as **3 characters** (wrong!)
- In UTF-8 locale: counts as **1 character** (correct)

**This is by design** - C locale is for ASCII-only text. Using it with UTF-8 gives incorrect results.

## Recommended Default Behavior

### Option 1: POSIX Compliant (Recommended for Compatibility)
```
wc file.txt (no flags):
- Check locale
- If UTF-8: use UTF-8 character-aware word counting
- If C: use byte-based counting
- Always output: lines words bytes
```

### Option 2: Modern UTF-8 First (Simpler)
```
wc file.txt (no flags):
- Always assume UTF-8
- Use SIMD UTF-8 counting
- Output: lines words chars
- Ignore locale (predictable behavior)
```

### Option 3: Locale-Aware (Matches GNU wc)
```
wc file.txt:
- Detect locale
- UTF-8 locale → UTF-8 processing → output chars
- C locale → byte processing → output bytes (chars = bytes)
```

## Testing Strategy

Test against standard `wc` with:

```bash
# UTF-8 text with multi-byte characters
wc file.txt
wc -lwm file.txt
wc -lwc file.txt

# Different locales
LC_ALL=C wc file.txt
LC_ALL=C wc -lwm file.txt
LC_ALL=en_US.UTF-8 wc file.txt
LC_ALL=en_US.UTF-8 wc -lwm file.txt

# Test files
- Pure ASCII
- UTF-8 with emoji, accents, Hebrew, Chinese
- Binary files
- Invalid UTF-8 sequences
```

## Final Recommendation

For this implementation:

1. **Support locale detection** (check `LC_CTYPE`, `LC_ALL`, `LANG`)
2. **UTF-8 locale**: Use existing SIMD UTF-8 implementation
3. **C locale**: Simple byte counting (bytes = chars)
4. **Default output**: `lines words bytes` (POSIX standard)
5. **With `-m` flag**: Show characters instead of bytes
6. **Document clearly**: Explain locale behavior and UTF-8 assumptions

This approach:
- ✓ Matches GNU `wc` behavior
- ✓ Leverages fast SIMD UTF-8 counting
- ✓ Falls back gracefully to byte counting
- ✓ Predictable and testable
- ✓ POSIX compliant

## Key Takeaways

1. **Default `wc` shows bytes, not characters**
2. **UTF-8 character counting = skip continuation bytes (10xxxxxx)**
3. **C locale is broken for UTF-8 text (by design)**
4. **Word counting depends on locale (ASCII vs Unicode whitespace)**
5. **Always compute both bytes and chars internally**
6. **Display based on flags and locale settings**

---

*Generated during debugging session on 2025-11-12*
