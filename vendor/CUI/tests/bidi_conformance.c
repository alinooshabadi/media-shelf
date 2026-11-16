/* Download BidiCharacterTest.txt separately; no external font or CUI needed. */
#include "../src/cui_bidi.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: cui_bidi_conformance BidiCharacterTest.txt\n"); return 2; }
    FILE *f = fopen(argv[1], "rb");
    if (!f) return 2;
    char text[65536];
    cui_bidi_char chars[4096];
    int order[4096], expected[4096], want_levels[4096];
    uint8_t levels[4096];
    int total = 0, passed = 0, skipped = 0, line = 0;
    while (fgets(text, sizeof text, f)) {
        line++;
        if (text[0] == '#' || text[0] == '\r' || text[0] == '\n') continue;
        char *fields[5], *p = text;
        int field = 0;
        fields[field++] = p;
        for (; *p; p++) if (*p == ';' && field < 5) { *p = 0; fields[field++] = p + 1; }
        if (field != 5) { fprintf(stderr, "invalid test line %d\n", line); return 2; }
        int n = 0, explicit = 0;
        p = fields[0];
        while (*p) {
            char *end;
            unsigned long cp = strtoul(p, &end, 16);
            if (end == p) break;
            if (n == 4096) return 2;
            chars[n++].cp = (uint32_t)cp;
            explicit |= (cp >= 0x202A && cp <= 0x202E) || (cp >= 0x2066 && cp <= 0x2069);
            p = end;
        }
        if (explicit) { skipped++; continue; }
        int direction = atoi(fields[1]);
        int base = cui__bidi_paragraph(chars, n, direction == 2 ? -1 : direction);
        cui__bidi_line(chars, 0, n, base, order, levels);
        int good = base == atoi(fields[2]);
        p = fields[3];
        for (int i = 0; i < n; i++) {
            while (*p == ' ') p++;
            if (*p == 'x') { want_levels[i] = -1; p++; }
            else { want_levels[i] = (int)strtol(p, &p, 10); good &= want_levels[i] == levels[i]; }
        }
        int m = 0;
        p = fields[4];
        for (;;) {
            char *end;
            long v = strtol(p, &end, 10);
            if (end == p) break;
            if (m == 4096) return 2;
            expected[m++] = (int)v;
            p = end;
        }
        int k = 0;
        for (int i = 0; i < n; i++) if (want_levels[order[i]] >= 0) {
            if (k >= m || expected[k] != order[i]) good = 0;
            k++;
        }
        good &= k == m;
        total++;
        passed += good;
        if (!good && total - passed <= 12) {
            printf("FAIL line %d base %d expected %d\ncodepoints:", line, base, atoi(fields[2]));
            for (int i = 0; i < n; i++) printf(" %04X", chars[i].cp);
            printf("\nlevels:"); for (int i = 0; i < n; i++) printf(" %d/%d", levels[i], want_levels[i]);
            printf("\norder:"); for (int i = 0; i < n; i++) if (want_levels[order[i]] >= 0) printf(" %d", order[i]);
            printf("\nexpected:"); for (int i = 0; i < m; i++) printf(" %d", expected[i]);
            puts("");
        }
    }
    fclose(f);
    printf("BidiCharacterTest: %d/%d passed (%.6f%%); %d explicit-format cases skipped\n", passed, total, total ? 100.0 * passed / total : 0, skipped);
    return passed == total && total > 0 ? 0 : 1;
}
