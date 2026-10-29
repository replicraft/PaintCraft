/* Independently decode indices/control with GIFLIB; do not composite pixels. */
#include <gif_lib.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char **argv){
    if(argc!=2)return 2;
    int error=0;GifFileType *gif=DGifOpenFileName(argv[1],&error);if(!gif||DGifSlurp(gif)==GIF_ERROR)return 3;
    if(gif->ImageCount!=1)return 4;
    SavedImage *frame=&gif->SavedImages[0];GifImageDesc *d=&frame->ImageDesc;
    ColorMapObject *palette=d->ColorMap?d->ColorMap:gif->SColorMap;
    GraphicsControlBlock control;if(DGifSavedExtensionToGCB(gif,0,&control)==GIF_ERROR)return 5;
    printf("{\"width\":%d,\"height\":%d,\"background\":%d,\"left\":%d,\"top\":%d,\"frame_width\":%d,\"frame_height\":%d,\"transparent\":%d,\"disposal\":%d,\"interlaced\":%s,\"global_palette\":[",gif->SWidth,gif->SHeight,gif->SBackGroundColor,d->Left,d->Top,d->Width,d->Height,control.TransparentColor,control.DisposalMode,d->Interlace?"true":"false");
    if(gif->SColorMap)for(int i=0;i<gif->SColorMap->ColorCount;i++)printf("%s[%d,%d,%d]",i?",":"",gif->SColorMap->Colors[i].Red,gif->SColorMap->Colors[i].Green,gif->SColorMap->Colors[i].Blue);
    printf("],\"palette\":[");for(int i=0;i<palette->ColorCount;i++)printf("%s[%d,%d,%d]",i?",":"",palette->Colors[i].Red,palette->Colors[i].Green,palette->Colors[i].Blue);
    printf("],\"indices\":[");for(int i=0;i<d->Width*d->Height;i++)printf("%s%u",i?",":"",frame->RasterBits[i]);puts("]}");
    DGifCloseFile(gif,&error);return 0;
}
