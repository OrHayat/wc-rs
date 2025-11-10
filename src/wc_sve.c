// sve_wc.c
#include <stdio.h>
#include <stdlib.h>
#include <arm_sve.h>


typedef struct {
    size_t lines;
    size_t words;
    size_t chars;
} FileStats;


// counts number of '\n' in a buffer using SVE
FileStats count_newlines_sve(const unsigned char *buf, size_t size) {
    FileStats stats = {
        .lines = 0,
        .words = 0,
        .chars = 0
    };
    size_t i = 0;

    while( i < size ) {
    // for (size_t i = 0; i < size; i += svcntw()) {
        // predicate for active lanes
        svbool_t pg = svwhilelt_b8(i, size);

        // load bytes with predicate
        svuint8_t data = svld1_u8(pg, buf + i);

        // compare bytes to '\n'
        svbool_t is_newline = svcmpeq_n_u8(pg, data, '\n');
        // // count matching lanes and add to total line count
        stats.lines += svcntp_b8(pg, is_newline);

        svbool_t is_space   = svcmpeq_n_u8(pg, data, ' ');
        svbool_t is_tab     = svcmpeq_n_u8(pg, data, '\t');
        svbool_t is_cr      = svcmpeq_n_u8(pg, data, '\r');
        svbool_t is_ff      = svcmpeq_n_u8(pg, data, 0x0C);
        svbool_t is_vt      = svcmpeq_n_u8(pg, data, 0x0B);

        // // combine whitespace masks
        svbool_t whitespace = svorr_b_z(pg, is_space, is_tab);
        whitespace = svorr_b_z(pg, whitespace, is_newline);
        whitespace = svorr_b_z(pg, whitespace, is_cr);
        whitespace = svorr_b_z(pg, whitespace, is_ff);
        whitespace = svorr_b_z(pg, whitespace, is_vt);
        // --- Identify UTF-8 character starts ---
        /*
        1. UTF-8 byte rules
        Single-byte ASCII: 0xxxxxxx → 1 char
        Multibyte sequences:
        2-byte: 110xxxxx 10xxxxxx
        3-byte: 1110xxxx 10xxxxxx 10xxxxxx
        4-byte: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
        Continuation bytes: 10xxxxxx → not the start of a char
        So the first byte of each UTF-8 character is the only one you count.
        */
        // Continuation bytes: 0b10xxxxxx = 0x80
        // Any byte != 0x80 in top two bits is start of UTF-8 char
        // create a vector with 0xC0 in all lanes 
        svuint8_t mask = svdup_n_u8(0xC0);

        // AND data with mask
        // svuint8_t masked = svand_u8_z(pg, data, mask);

        svuint8_t masked = svand_u8_z(pg, data, svdup_n_u8(0xC0));
        svbool_t char_start = svcmpne_n_u8(pg, masked, 0x80);
        stats.chars += svcntp_b8(pg, char_start);

        // // // compare masked != 0x80 to get char_start
        // svbool_t char_start = svcmpne_n_u8(pg, masked, 0x80);
        // // // --- Count UTF-8 chars inside words ---
        // svbool_t char_in_word = svand_b_z(pg, char_start, svnot_b_z(pg, whitespace));

        // // svbool_t char_in_word = svand_b8(pg, char_start, svnot_b8(pg, whitespace));
        // stats.chars += svcntp_b8(pg, char_in_word);

        // --- Count UTF-8 characters ---
        // UTF-8 character = byte that is NOT a continuation byte (top bits != 10)
        // Continuation byte pattern = 0b10xxxxxx = 0x80
        // svbool_t char_start = svcmpne_n_u8(pg, svand_n_u8(data, 0xC0), 0x80);
        // stats.chars += svcntp_b8(pg, char_start);

        // svbool_t active_spaces = svcmpeq_n_u8(pg, data, ' ');
        // svbool_t active_tabs = svcmpeq_n_u8(pg, data, '\t');
        // svbool_t active_crs = svcmpeq_n_u8(pg, data, '\r');
        // svbool_t active_ffs = svcmpeq_n_u8(pg, data, 0x0C);
        // svbool_t active_vts = svcmpeq_n_u8(pg, data, 0x0B);
        // svorr_b8(pg, active_spaces, active_tabs);
        // svbool_t whitespace_mask = active_newlines;
        // whitespace_mask = svorr_b8(pg, whitespace_mask, active_spaces);
        // whitespace_mask = svorr_b8(pg, whitespace_mask, active_tabs);
        // whitespace_mask = svorr_b8(pg, whitespace_mask, active_crs);
        // whitespace_mask = svorr_b8(pg, whitespace_mask, active_ffs);
        // whitespace_mask = svorr_b8(pg, whitespace_mask, active_vts);

        // svbool_t non_ws_mask = svnot_b8(pg, whitespace_mask);
        
        // // shift non_ws_mask right by 1 byte to align with current byte
        // svbool_t prev_non_ws = svext_b8(non_ws_mask, svdup_n_u8(0), 1);

        // // word_end = current byte is whitespace AND previous byte was non-whitespace
        // svbool_t word_end = svand_b8(pg, whitespace_mask, prev_non_ws);

        // // count word ends in this vector chunk
        // uint64_t count = svcntp_b8(pg, word_end);

        // advance by number of active bytes in the vector
        i += svcntp_b8(pg, pg);   // count active lanes in pg
        // i += svcntp_b8(svptrue_b8(), pg); //same code

        // i+=svcntw();
    }

    return stats;
}

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s <file>\n", argv[0]);
        return 1;
    }

    FILE *f = fopen(argv[1], "rb");
    if (!f) {
        perror("fopen");
        return 1;
    }

    // get file size
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    rewind(f);

    unsigned char *buf = malloc(size);
    if (!buf) {
        fprintf(stderr, "malloc failed\n");
        fclose(f);
        return 1;
    }

    if (fread(buf, 1, size, f) != (size_t)size) {
        perror("fread");
        free(buf);
        fclose(f);
        return 1;
    }
    fclose(f);

    FileStats stats = count_newlines_sve(buf, size);
    printf("%zu %zu\n", stats.lines, stats.chars);

    free(buf);
    return 0;
}
