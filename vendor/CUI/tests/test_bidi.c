/* A wrapped line must keep paragraph context, while L1 resets its own spaces. */
#include "../src/cui_bidi.h"
#include <stdio.h>

#define CHECK(x) do { if (!(x)) { fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #x); return 1; } } while (0)
int main(void) {
    CHECK(cui__bidi_strong_direction('A') == 0);
    CHECK(cui__bidi_strong_direction(0x628) == 1);
    CHECK(cui__bidi_strong_direction(0x5d0) == 1);
    CHECK(cui__bidi_strong_direction('1') == -1);
    CHECK(cui__bidi_strong_direction('(') == -1);
    CHECK(cui__bidi_paragraph(NULL, 0, -1) == 0);
    cui__bidi_line(NULL, 0, 0, 0, NULL, NULL);
    cui_bidi_char c[] = { {0x627}, {' '}, {'1'}, {'2'}, {' '}, {'a'}, {' '}, {0x628} };
    int order[8];
    uint8_t levels[8];
    int base = cui__bidi_paragraph(c, 8, -1);
    CHECK(base == 1 && c[2].level == 2 && c[3].level == 2);
    cui__bidi_line(c, 2, 3, base, order, levels);
    CHECK(order[0] == 4 && order[1] == 2 && order[2] == 3);
    CHECK(levels[2] == 1 && c[2].level == 2);
    cui__bidi_line(c, 5, 2, base, order, levels);
    CHECK(order[0] == 6 && order[1] == 5 && c[5].level == 2);
    cui__bidi_line(c, 0, 8, base, order, levels);
    CHECK(cui__bidi_mirror('(', 1) == ')' && cui__bidi_mirror('(', 2) == '(');
    CHECK(cui__bidi_mirror(0x2264, 1) == 0x2265);
    CHECK(cui__bidi_mirror(0x627, 1) == 0x627);
    puts("paragraph context, line L1/L2 and L4 checks passed");
    return 0;
}
