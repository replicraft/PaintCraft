#include <CoreFoundation/CoreFoundation.h>
#include <CoreGraphics/CoreGraphics.h>
#include <ImageIO/ImageIO.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char**argv){
 setvbuf(stdout,NULL,_IONBF,0);
 for(int arg=1;arg<argc;arg++){
  CFURLRef url=CFURLCreateFromFileSystemRepresentation(NULL,(const UInt8*)argv[arg],strlen(argv[arg]),false);
  CGImageSourceRef source=CGImageSourceCreateWithURL(url,NULL);CFRelease(url);
  if(!source){fprintf(stderr,"refused %s\n",argv[arg]);return 2;}
  size_t count=CGImageSourceGetCount(source);printf("SOURCE %s COUNT %zu\n",argv[arg],count);
  for(size_t index=0;index<count;index++){
   CFDictionaryRef properties=CGImageSourceCopyPropertiesAtIndex(source,index,NULL);
   if(properties)CFRelease(properties);
   printf("DECODE %zu\n",index);
   CGImageRef image=CGImageSourceCreateImageAtIndex(source,index,NULL);if(!image){puts("NOIMAGE");continue;}
   size_t w=CGImageGetWidth(image),h=CGImageGetHeight(image);
   CFDataRef raw=CGDataProviderCopyData(CGImageGetDataProvider(image));
   printf("IMAGE %zu %zux%zu bpp=%zu bpc=%zu row=%zu info=%u alpha=%u rawlength=%ld\n",index,w,h,CGImageGetBitsPerPixel(image),CGImageGetBitsPerComponent(image),CGImageGetBytesPerRow(image),(unsigned)CGImageGetBitmapInfo(image),(unsigned)CGImageGetAlphaInfo(image),(long)CFDataGetLength(raw));
   if(w<=32&&h<=32){
    size_t n=w*h*4;unsigned char *bytes=calloc(n,1);CGColorSpaceRef space=CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef context=CGBitmapContextCreate(bytes,w,h,8,w*4,space,kCGImageAlphaPremultipliedLast);
    CGContextDrawImage(context,CGRectMake(0,0,w,h),image);
    char name[4096];snprintf(name,sizeof(name),"%s.image%zu.rgba",argv[arg],index);FILE*f=fopen(name,"wb");fwrite(bytes,1,n,f);fclose(f);
    snprintf(name,sizeof(name),"%s.image%zu.provider",argv[arg],index);f=fopen(name,"wb");fwrite(CFDataGetBytePtr(raw),1,CFDataGetLength(raw),f);fclose(f);
    CGContextRelease(context);CGColorSpaceRelease(space);free(bytes);
   }
   CFRelease(raw);CGImageRelease(image);
  }
  CFRelease(source);
 }
 return 0;
}
