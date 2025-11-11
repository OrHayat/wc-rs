use std::ops::BitAnd;

use crate::FileCounts;

pub fn word_count_scalar(content: &[u8]) -> FileCounts {
    let mut res = FileCounts {
        lines: 0,
        words: 0,
        bytes: content.len(),
        chars: 0,
    };
    let mut seen_space = true;

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
//fallback implementation for word count - scalar version
// pub fn word_count_scalar(content: &str) -> FileCounts {
//     let mut lines = 0;
//     let mut words = 0;
//     let mut chars = 0;
//     let mut seen_space = true;

//     for ch in content.chars() {
//         chars += 1;

//         if ch == '\n' {
//             lines += 1;
//         }

//         if ch.is_whitespace() {
//             seen_space = true;
//         } else if seen_space {
//             words += 1;
//             seen_space = false;
//         }
//     }

//     FileCounts {
//         lines,
//         words,
//         bytes: content.len(),
//         chars,
//     }
// }

pub fn is_whitespace_byte(ch: char) -> bool {
    matches!(ch, ' ' | '\n' | '\t' | '\r' | '\x0B' | '\x0C')
}
