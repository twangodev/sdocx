/* Test adapter for the externally supplied, hash-pinned NanoSVG headers. */
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
    NSVGrasterizer *r = nsvgCreateRasterizer();
    r->width=n; r->height=n; r->stride=n*4;
    r->scanline=malloc(n); r->cscanline=n; r->bitmap=calloc(n*n,4);
    NSVGcachedPaint paint = {0}; paint.type=NSVG_PAINT_COLOR; paint.colors[0]=0xff000000;
    for (NSVGshape *s=image->shapes; s; s=s->next) {
        if (!(s->flags & NSVG_FLAGS_VISIBLE) || s->stroke.type == NSVG_PAINT_NONE) continue;
        nsvg__resetPool(r); r->freelist=NULL; r->nedges=0;
        nsvg__flattenShapeStroke(r,s,(float)n/120);
        for (int i=0;i<r->nedges;i++) { r->edges[i].y0*=5; r->edges[i].y1*=5; }
        qsort(r->edges,r->nedges,sizeof(NSVGedge),nsvg__cmpEdge);
        memset(r->bitmap,0,n*n*4);
        nsvg__rasterizeSortedEdges(r,0,0,(float)n/120,&paint,NSVG_FILLRULE_NONZERO);
        printf("%d %d",n,r->nedges);
        for (int i=0;i<r->nedges;i++) {
            NSVGedge e=r->edges[i];
            printf(" %.9g %.9g %.9g %.9g %d",e.x0,e.y0,e.x1,e.y1,e.dir);
        }
        putchar('\n');
        for (int i=0;i<n*n;i++) printf("%02x",r->bitmap[4*i+3]);
        putchar('\n');
    }
    free(r->bitmap); r->bitmap=NULL; nsvgDeleteRasterizer(r); nsvgDelete(image);
}
