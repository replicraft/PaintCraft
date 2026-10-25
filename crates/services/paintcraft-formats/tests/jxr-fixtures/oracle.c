/* PaintCraft-authored independent JXRLib fixture/oracle harness. */
#include "JXRGlue.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void check(ERR e) { if(e) {fprintf(stderr,"JXRLib error %d\n",(int)e);exit(3);} }
int main(int argc,char **argv) {
 if(argc<5)return 2;
 unsigned w=(unsigned)atoi(argv[2]),h=(unsigned)atoi(argv[3]);
 if(!w||!h||w*h>1048576)return 2;
 unsigned char *rgb=calloc(w*h,3); if(!rgb)return 2;
 if(!strcmp(argv[1],"encode")) {
  for(unsigned y=0;y<h;y++)for(unsigned x=0;x<w;x++)for(unsigned c=0;c<3;c++)rgb[(y*w+x)*3+c]=(x*47+y*13+c*71)%256;
  struct WMPStream *stream=NULL;PKImageEncode *enc=NULL;
  check(CreateWS_File(&stream,argv[4],"w+b"));check(PKImageEncode_Create_WMP(&enc));
  CWMIStrCodecParam params={0};params.cfColorFormat=YUV_444;
  check(enc->Initialize(enc,stream,&params,sizeof(params)));
  check(enc->SetPixelFormat(enc,GUID_PKPixelFormat24bppRGB));check(enc->SetSize(enc,w,h));
  check(enc->WritePixels(enc,h,rgb,w*3));check(enc->Release(&enc));
 } else if(!strcmp(argv[1],"decode")) {
  PKImageDecode *dec=NULL;I32 dw=0,dh=0;
  check(PKCodecFactory_CreateDecoderFromFile(argv[4],&dec));check(dec->GetSize(dec,&dw,&dh));
  if(dw!=(int)w||dh!=(int)h)return 4;
  PKRect rect={0,0,(I32)w,(I32)h};check(dec->Copy(dec,&rect,rgb,w*3));check(dec->Release(&dec));
 } else return 2;
 if(argc>5) {FILE *out=fopen(argv[5],"wb");if(!out)return 5;fwrite(rgb,1,w*h*3,out);fclose(out);}
 free(rgb);return 0;
}
