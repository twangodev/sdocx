#define NANOSVG_IMPLEMENTATION
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvg.h"
#include "nanosvgrast.h"
#include <stdio.h>
int main(int argc,char **argv){int n=atoi(argv[2]); NSVGimage *im=nsvgParseFromFile(argv[1],"px",96);if(!im)return 1; unsigned char *p=malloc(n*n*4); NSVGrasterizer *r=nsvgCreateRasterizer(); nsvgRasterize(r,im,0,0,(float)n/120,p,n,n,n*4); fwrite(p,4,n*n,stdout);return 0;}
