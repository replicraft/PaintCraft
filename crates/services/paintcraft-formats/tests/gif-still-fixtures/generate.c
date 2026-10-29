/* Owned GIFLIB 6.1.3 fixture encoder. No PaintCraft encoder/decoder is used. */
#include <gif_lib.h>
#include <stdio.h>
#include <stdlib.h>
static void write_case(const char *root, const char *name, int bg, int transparent, int local, int no_global, int disposal, int interlace) {
    char path[4096];
    if (snprintf(path,sizeof(path),"%s/%s.gif",root,name)<0) abort();
    int error=0;
    GifFileType *gif=EGifOpenFileName(path,0,&error);
    if (!gif) abort();
    EGifSetGifVersion(gif,1);
    GifColorType colors[4]={{17,34,51},{90,80,70},{200,40,60},{5,210,99}};
    GifColorType other[4]={{13,71,201},{80,19,222},{210,99,5},{4,6,8}};
    ColorMapObject *global=GifMakeMapObject(4,colors), *palette=GifMakeMapObject(4,other);
    int height=interlace?11:4, frame_h=interlace?9:2;
    if (EGifPutScreenDesc(gif,5,height,8,bg,no_global?NULL:global)==GIF_ERROR) abort();
    unsigned char gce[4]={(unsigned char)((disposal<<2)|(transparent>=0)),0,0,(unsigned char)(transparent<0?0:transparent)};
    if (EGifPutExtension(gif,GRAPHICS_EXT_FUNC_CODE,4,gce)==GIF_ERROR) abort();
    if (EGifPutImageDesc(gif,1,1,3,frame_h,interlace,local?palette:NULL)==GIF_ERROR) abort();
    unsigned char data[9][3];
    for(int y=0;y<frame_h;y++) for(int x=0;x<3;x++) data[y][x]=(unsigned char)((y*3+x)%4);
    if (interlace) {
        int starts[4]={0,4,2,1}, steps[4]={8,8,4,2};
        for(int p=0;p<4;p++) for(int y=starts[p];y<frame_h;y+=steps[p]) if(EGifPutLine(gif,data[y],3)==GIF_ERROR) abort();
    } else for(int y=0;y<frame_h;y++) if(EGifPutLine(gif,data[y],3)==GIF_ERROR) abort();
    if(EGifCloseFile(gif,&error)==GIF_ERROR) abort();
    GifFreeMapObject(global);GifFreeMapObject(palette);
}
int main(int argc,char **argv) {
    if(argc!=2)return 2;
    write_case(argv[1],"offset-background0",0,-1,0,0,0,0);
    write_case(argv[1],"offset-background2",2,-1,0,0,0,0);
    write_case(argv[1],"offset-transparent",2,1,0,0,0,0);
    write_case(argv[1],"offset-local-palette",2,-1,1,0,0,0);
    write_case(argv[1],"offset-local-only",0,-1,1,1,0,0);
    write_case(argv[1],"offset-local-transparent",0,1,1,1,0,0);
    write_case(argv[1],"offset-disposal1",2,-1,0,0,1,0);
    write_case(argv[1],"offset-disposal2",2,-1,0,0,2,0);
    write_case(argv[1],"offset-disposal3",2,-1,0,0,3,0);
    write_case(argv[1],"offset-interlace",2,-1,0,0,0,1);
    return 0;
}
