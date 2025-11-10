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
        svbool_t active_newlines = svcmpeq_n_u8(pg, data, '\n');

        // // count matching lanes and add to total line count
        stats.lines += svcntp_b8(pg, active_newlines);
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
    printf("%zu\n", stats.lines);

    free(buf);
    return 0;
}
