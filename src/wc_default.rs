use crate::FileCounts;

pub fn word_count_scalar(content: &str) -> FileCounts {
    word_count_scalar_with_state(content.as_bytes(), true)
}

pub(crate) fn word_count_scalar_with_state(content: &[u8], initial_seen_space: bool) -> FileCounts {
    let mut res = FileCounts {
        lines: 0,
        words: 0,
        bytes: content.len(),
        chars: 0,
    };
    let mut seen_space = initial_seen_space;

    for ch in content {
        if *ch == '\n' as u8 {
            res.lines += 1;
        }
        if *ch & 0b11000000u8 != 0b11000000u8 {
            res.chars += 1;
        }
        if is_whitespace_byte(*ch as char) {
            seen_space = true;
        } else if seen_space {
            res.words += 1;
            seen_space = false;
        }
    }
    res
}

pub(crate) fn is_whitespace_byte(ch: char) -> bool {
    ch.is_whitespace()
}
