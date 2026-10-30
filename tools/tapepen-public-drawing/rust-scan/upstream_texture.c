/* Full-texture oracle using externally supplied, hash-pinned NanoSVG headers. */
#define NANOSVG_IMPLEMENTATION
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvg.h"
#include "nanosvgrast.h"
#include <stdio.h>
int main(int argc, char **argv) {
    if (argc != 3) return 2;
    int n = atoi(argv[2]);
    if (n <= 0 || n > 1024) return 2;
    NSVGimage *image = nsvgParseFromFile(argv[1], "px", 96);
    if (!image) return 1;
    unsigned char *pixels = malloc(n*n*4);
    NSVGrasterizer *r = nsvgCreateRasterizer();
    nsvgRasterize(r, image, 0, 0, (float)n/120, pixels, n, n, n*4);
    fwrite(pixels, 4, n*n, stdout);
    free(pixels); nsvgDeleteRasterizer(r); nsvgDelete(image);
}
