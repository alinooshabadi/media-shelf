/* Oracle adapter: one UTF-8 line in, glyph/advance/x/y records out. */
#include "../src/cui_shape.h"
#include "../src/cui_bidi.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char **argv) {
    if (argc<2) return 2;
    FILE *file=fopen(argv[1],"rb"); if (!file) return 2;
    fseek(file,0,SEEK_END); long size=ftell(file); rewind(file);
    if (size<=0) return 2;
    void *bytes=malloc((size_t)size); if (!bytes) return 2;
    if (fread(bytes,1,(size_t)size,file)!=(size_t)size) return 2;
    fclose(file); cui_shape_font font;
    if (!cui__shape_font_init(&font,bytes,(size_t)size,0)) return 2;
    char line[16384];
    while (fgets(line,sizeof line,stdin)) {
        cui_shape_glyph g[8192]; int n=0;
        for (int i=0;line[i] && line[i]!='\n';) {
            unsigned char b=(unsigned char)line[i]; uint32_t cp=b; int start=i++;
            int extra=b>=0xf0?3:b>=0xe0?2:b>=0xc0?1:0;
            if (extra) { cp=b&((1u<<(6-extra))-1); while(extra-- && line[i]) cp=cp*64+((unsigned char)line[i++]&63); }
            uint32_t mirror=cui__bidi_mirror(cp, 1);
            g[n++]=(cui_shape_glyph){mirror?mirror:cp,0,start,0,0,0,0,0,0};
        }
        n=cui__shape(&font,g,n,8192,0x61726162,1,0);
        printf("%d",n);
        for (int i=0;i<n;i++) printf(" %d,%d,%d,%d",g[i].id,g[i].advance,g[i].x,g[i].y);
        putchar('\n');
    }
    free(bytes); return 0;
}
