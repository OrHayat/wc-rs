// sve_wc.c
#include <stdio.h>
#include <stdlib.h>
#include <arm_sve.h>
#include <stdbool.h>



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

    bool prev_was_whitespace = true;// assume start of file is whitespace so first char is word start
    
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
        // Create a vector where every lane has 0xC0
        // 0xC0 = 11000000b, used to extract the top 2 bits of each byte
        svuint8_t mask = svdup_n_u8(0xC0);

        // AND each byte in 'data' with mask to keep only the top 2 bits
        // For ASCII: 0x00–0x7F → top bits = 00 → masked = 0x00
        // For continuation bytes: 10xxxxxx → masked = 0x80
        // For first byte of multi-byte UTF-8: 11xxxxxx → masked = 0xC0, 0xE0, etc.
        svuint8_t masked = svand_u8_z(pg, data, mask);
        // Compare masked value != 0x80
        // Only true for first bytes of UTF-8 characters
        // ASCII bytes (masked = 0x00) → true
        // Continuation bytes (masked = 0x80) → false
        svbool_t char_start = svcmpne_n_u8(pg, masked, 0x80);
        stats.chars += svcntp_b8(pg, char_start);

        svuint8_t ws_u8 = svsel_u8(whitespace, svdup_n_u8(1), svdup_n_u8(0));

                // prev_u8: 1 for previous lane whitespace, 0 otherwise (vector of u8)
        svuint8_t prev_u8 = svext_u8(
            svdup_n_u8(prev_was_whitespace ? 1 : 0),  // insert previous
            ws_u8,                                    // current ws as u8
            1
        );

        // Convert prev_u8 to predicate: true where prev was whitespace
        svbool_t prev_ws = svcmpne_n_u8(pg, prev_u8, 0);

        // Current lane non-whitespace predicate
        svbool_t curr_non_ws = svcmpne_n_u8(pg, ws_u8, 1);  // ws_u8: 1 = whitespace

        // Word start = previous whitespace AND current non-whitespace
        svbool_t word_start = svand_b_z(pg, prev_ws, curr_non_ws);

        // Count words
        stats.words += svcntp_b8(pg, word_start);

        // Update prev_was_whitespace for next iteration
        prev_was_whitespace = svlastb_u8(pg, ws_u8) != 0;




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
    printf("%zu %zu %zu\n", stats.lines, stats.chars, stats.words);

    free(buf);
    return 0;
}
