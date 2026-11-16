/* Extended gallery pages: the newer components, one page per family. */
#ifndef GALLERY_MORE_H
#define GALLERY_MORE_H

#include <cui/cui.h>

int gallery_more_count(void);
const char *gallery_more_name(int page);
const char *gallery_more_icon(int page);
void gallery_more_page(cui_ctx *ui, int page);

#endif
