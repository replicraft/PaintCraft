/* Independent Apple ImageIO still-image oracle; outputs premultiplied RGBA. */
#include <CoreFoundation/CoreFoundation.h>
#include <CoreGraphics/CoreGraphics.h>
#include <ImageIO/ImageIO.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char **argv) {
    if(argc!=2)return 2;
    CFURLRef url=CFURLCreateFromFileSystemRepresentation(NULL,(const UInt8*)argv[1],strlen(argv[1]),0);
    CGImageSourceRef source=CGImageSourceCreateWithURL(url,NULL);
    CGImageRef image=CGImageSourceCreateImageAtIndex(source,0,NULL);
    if(!image)return 3;
    size_t w=CGImageGetWidth(image),h=CGImageGetHeight(image);
    unsigned char *pixels=calloc(w*h,4);
    CGColorSpaceRef color=CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef ctx=CGBitmapContextCreate(pixels,w,h,8,w*4,color,kCGImageAlphaPremultipliedLast|kCGBitmapByteOrder32Big);
    CGContextDrawImage(ctx,CGRectMake(0,0,w,h),image);
    printf("{\"width\":%zu,\"height\":%zu,\"rgba\":[",w,h);
    for(size_t i=0;i<w*h*4;i++)printf("%s%u",i?",":"",pixels[i]);
    puts("]}");
    CGContextRelease(ctx);CGColorSpaceRelease(color);CGImageRelease(image);CFRelease(source);CFRelease(url);free(pixels);return 0;
}
