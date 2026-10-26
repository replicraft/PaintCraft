/* PaintCraft-owned independent AV1 fixture oracle, no product decoder route. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "aom/aom_decoder.h"
#include "aom/aomdx.h"
int main(int argc,char **argv) {
    if(argc!=3)return 2;
    FILE *input=fopen(argv[1],"rb");if(!input)return 3;
    if(fseek(input,0,SEEK_END))return 4;
    long length=ftell(input);if(length<=0||length>64*1024*1024)return 5;
    rewind(input);
    uint8_t *bytes=malloc((size_t)length);if(!bytes)return 6;
    if(fread(bytes,1,(size_t)length,input)!=(size_t)length)return 7;
    fclose(input);
    aom_codec_ctx_t codec={0};
    aom_codec_dec_cfg_t cfg={.threads=1,.allow_lowbitdepth=1};
    if(aom_codec_dec_init(&codec,aom_codec_av1_dx(),&cfg,0)!=AOM_CODEC_OK)return 8;
    if(aom_codec_decode(&codec,bytes,(size_t)length,NULL)!=AOM_CODEC_OK)return 9;
    aom_codec_iter_t iter=NULL;
    aom_image_t *image=aom_codec_get_frame(&codec,&iter);
    if(!image||image->bit_depth!=8||!image->d_w||!image->d_h)return 10;
    size_t area=(size_t)image->d_w*image->d_h;if(area>4194304)return 11;
    uint8_t *planes=calloc(area,3);if(!planes)return 12;
    for(unsigned p=0;p<(image->monochrome?1u:3u);p++) {
        unsigned sx=p?image->x_chroma_shift:0,sy=p?image->y_chroma_shift:0;
        unsigned w=(image->d_w+((1u<<sx)-1))>>sx,h=(image->d_h+((1u<<sy)-1))>>sy;
        if(!image->planes[p]||image->stride[p]<(int)w)return 13;
        for(unsigned y=0;y<h;y++)memcpy(planes+p*area+(size_t)y*w,image->planes[p]+(size_t)y*image->stride[p],w);
    }
    char path[4096];
    if(snprintf(path,sizeof(path),"%s.planes",argv[2])>=(int)sizeof(path))return 14;
    FILE *out=fopen(path,"wb");if(!out)return 15;
    if(fwrite(planes,area,3,out)!=3)return 16;fclose(out);
    printf("{\"reference\":\"libaom\",\"version\":\"%s\",\"width\":%u,\"height\":%u,\"depth\":%u,\"monochrome\":%s,\"subsampling_x\":%u,\"subsampling_y\":%u,\"primaries\":%u,\"transfer\":%u,\"matrix\":%u,\"full_range\":%s}\n",aom_codec_version_str(),image->d_w,image->d_h,image->bit_depth,image->monochrome?"true":"false",image->x_chroma_shift,image->y_chroma_shift,image->cp,image->tc,image->mc,image->range==AOM_CR_FULL_RANGE?"true":"false");
    if(aom_codec_get_frame(&codec,&iter))return 17;
    free(planes);free(bytes);aom_codec_destroy(&codec);return 0;
}
