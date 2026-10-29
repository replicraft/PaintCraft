/* Authored fixture producer and raw-sample oracle. No TIFFReadRGBAImage APIs:
 * those convenience paths can premultiply alpha and change source samples.
 * Build only as a qualification tool; libtiff is not a PaintCraft dependency. */
#include <tiffio.h>
#include <lcms2.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void fail(const char *message) {
	fprintf(stderr, "%s\n", message);
	exit(1);
}
static const uint8_t rgba8[] = {
	10,20,30,0,40,50,60,1,70,80,90,127,
	100,110,120,128,130,140,150,254,160,170,180,255
};
static const uint16_t rgba16[] = {
	129,255,256,0,43981,39030,30292,32896,
	65535,1,32767,65535,256,32768,65534,256
};
static uint32_t sample(uint32_t x, uint32_t y, uint32_t channel,
	uint32_t width, uint16_t bits, uint16_t channels, int seed) {
	if (seed == 100) return (uint32_t[]){23,101,209}[channel % 3];
	if (bits == 1) return (x + y + (uint32_t)seed) & 1;
	if (seed == 8 && channels == 4 && bits == 8)
		return rgba8[((y * width + x) * 4 + channel) % sizeof(rgba8)];
	if (seed == 16 && channels == 4 && bits == 16)
		return rgba16[((y * width + x) * 4 + channel) % 16];
	uint32_t value = (x * 19 + y * 37 + channel * 53 + seed * 7) & 255;
	if (channel == channels - 1 && channels != 1 && channels != 3)
		value = (uint32_t[]){0,1,137,255}[(x + y) % 4];
	return bits == 16 ? value * 257 + (value < 255 && !((x + channel) & 1) ? 1 : 0) : value;
}
static void store(void *buffer, size_t index, uint32_t value, uint16_t bits) {
	if (bits == 8) ((uint8_t *)buffer)[index] = (uint8_t)value;
	else ((uint16_t *)buffer)[index] = (uint16_t)value;
}
static void page(TIFF *tiff, uint32_t width, uint32_t height, uint16_t bits,
	uint16_t channels, uint16_t photo, uint16_t alpha, uint16_t compression,
	int tiled, int seed) {
	TIFFSetField(tiff, TIFFTAG_IMAGEWIDTH, width);
	TIFFSetField(tiff, TIFFTAG_IMAGELENGTH, height);
	TIFFSetField(tiff, TIFFTAG_BITSPERSAMPLE, bits);
	TIFFSetField(tiff, TIFFTAG_SAMPLESPERPIXEL, channels);
	TIFFSetField(tiff, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
	TIFFSetField(tiff, TIFFTAG_PHOTOMETRIC, photo);
	TIFFSetField(tiff, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
	TIFFSetField(tiff, TIFFTAG_ORIENTATION, ORIENTATION_TOPLEFT);
	TIFFSetField(tiff, TIFFTAG_COMPRESSION, compression);
	if (compression == COMPRESSION_JPEG) TIFFSetField(tiff, TIFFTAG_JPEGQUALITY, 100);
	TIFFSetField(tiff, TIFFTAG_XRESOLUTION, 96.0);
	TIFFSetField(tiff, TIFFTAG_YRESOLUTION, 72.0);
	TIFFSetField(tiff, TIFFTAG_RESOLUTIONUNIT, RESUNIT_INCH);
	TIFFSetField(tiff, TIFFTAG_SOFTWARE, "PaintCraft authored libtiff fixture");
	if (photo == PHOTOMETRIC_SEPARATED) TIFFSetField(tiff, TIFFTAG_INKSET, INKSET_CMYK);
	if (alpha) TIFFSetField(tiff, TIFFTAG_EXTRASAMPLES, 1, &alpha);
	if (tiled) {
		TIFFSetField(tiff, TIFFTAG_TILEWIDTH, 16);
		TIFFSetField(tiff, TIFFTAG_TILELENGTH, 16);
		tmsize_t size = TIFFTileSize(tiff);
		void *buffer = calloc(1, (size_t)size);
		if (!buffer) fail("tile allocation");
		for (uint32_t ty = 0; ty < height; ty += 16) {
			for (uint32_t tx = 0; tx < width; tx += 16) {
				memset(buffer, 0, (size_t)size);
				for (uint32_t y = 0; y < 16 && ty + y < height; y++)
					for (uint32_t x = 0; x < 16 && tx + x < width; x++)
						for (uint16_t c = 0; c < channels; c++)
							store(buffer, ((size_t)y * 16 + x) * channels + c,
								sample(tx+x,ty+y,c,width,bits,channels,seed), bits);
				if (TIFFWriteTile(tiff, buffer, tx, ty, 0, 0) < 0) fail("write tile");
			}
		}
		free(buffer);
	} else {
		TIFFSetField(tiff, TIFFTAG_ROWSPERSTRIP, compression == COMPRESSION_JPEG ? 16 : 1);
		tmsize_t size = TIFFScanlineSize(tiff);
		void *buffer = calloc(1, (size_t)size);
		if (!buffer) fail("scanline allocation");
		for (uint32_t y = 0; y < height; y++) {
			memset(buffer, 0, (size_t)size);
			for (uint32_t x = 0; x < width; x++) {
				for (uint16_t c = 0; c < channels; c++) {
					uint32_t value = sample(x,y,c,width,bits,channels,seed);
					if (alpha == EXTRASAMPLE_ASSOCALPHA && c < channels - 1) {
						uint32_t opacity = sample(x,y,channels-1,width,bits,channels,seed);
						if (value > opacity) value = opacity;
					}
					if (bits == 1) ((uint8_t *)buffer)[x/8] |= value << (7 - x%8);
					else store(buffer, (size_t)x * channels + c, value, bits);
				}
			}
			if (TIFFWriteScanline(tiff, buffer, y, 0) < 0) fail("write scanline");
		}
		free(buffer);
	}
	if (!TIFFWriteDirectory(tiff)) fail("write directory");
}
static TIFF *open_output(const char *directory, const char *name, const char *mode) {
	char path[4096];
	if (snprintf(path,sizeof(path),"%s/%s",directory,name) >= (int)sizeof(path)) fail("output path");
	TIFF *tiff = TIFFOpen(path, mode);
	if (!tiff) fail("open output TIFF");
	return tiff;
}
static void float_page(TIFF *tiff) {
	uint32_t samples[] = {0x7fc01234,0x7f800000,0xbf000000,0x3f800000};
	TIFFSetField(tiff,TIFFTAG_IMAGEWIDTH,2);
	TIFFSetField(tiff,TIFFTAG_IMAGELENGTH,2);
	TIFFSetField(tiff,TIFFTAG_BITSPERSAMPLE,32);
	TIFFSetField(tiff,TIFFTAG_SAMPLESPERPIXEL,1);
	TIFFSetField(tiff,TIFFTAG_SAMPLEFORMAT,SAMPLEFORMAT_IEEEFP);
	TIFFSetField(tiff,TIFFTAG_PHOTOMETRIC,PHOTOMETRIC_MINISBLACK);
	TIFFSetField(tiff,TIFFTAG_PLANARCONFIG,PLANARCONFIG_CONTIG);
	TIFFSetField(tiff,TIFFTAG_ORIENTATION,ORIENTATION_TOPLEFT);
	TIFFSetField(tiff,TIFFTAG_COMPRESSION,COMPRESSION_NONE);
	TIFFSetField(tiff,TIFFTAG_ROWSPERSTRIP,1);
	for(uint32_t y=0;y<2;y++) if(TIFFWriteScanline(tiff,samples+y*2,y,0)<0) fail("write float scanline");
	if(!TIFFWriteDirectory(tiff)) fail("write float directory");
}
static void srgb_profile(TIFF *tiff) {
	cmsHPROFILE profile=cmsCreate_sRGBProfile(); if(!profile) fail("create reference sRGB ICC");
	cmsUInt32Number size=0; if(!cmsSaveProfileToMem(profile,NULL,&size)) fail("ICC size");
	unsigned char *bytes=malloc(size); if(!bytes || !cmsSaveProfileToMem(profile,bytes,&size)) fail("ICC bytes");
	_Static_assert(offsetof(cmsICCHeader,date)==24,"published ICC date offset");
	uint16_t date[]={2026,10,29,0,0,0};
	for(size_t i=0;i<6;i++){bytes[24+i*2]=(unsigned char)(date[i]>>8);bytes[25+i*2]=(unsigned char)date[i];}
	cmsHPROFILE validated=cmsOpenProfileFromMem(bytes,size);
	if(!validated || cmsGetColorSpace(validated)!=cmsSigRgbData) fail("validate authored ICC");
	TIFFSetField(tiff,TIFFTAG_ICCPROFILE,size,bytes);
	cmsCloseProfile(validated);cmsCloseProfile(profile);free(bytes);
}
static void generate(const char *directory) {
	TIFF *tiff = open_output(directory,"rgba8-le-strips-none.tif","wl");
	page(tiff,3,2,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_NONE,0,8);
	page(tiff,2,1,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_NONE,0,9);
	TIFFClose(tiff);
	tiff = open_output(directory,"rgba16-be-strips-lzw.tif","wb");
	page(tiff,2,2,16,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_LZW,0,16);
	page(tiff,1,1,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_LZW,0,8);
	TIFFClose(tiff);
	tiff = open_output(directory,"rgba8-le-tiles-deflate.tif","wl");
	page(tiff,17,19,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_ADOBE_DEFLATE,1,3);
	page(tiff,9,3,16,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_ADOBE_DEFLATE,1,7);
	TIFFClose(tiff);
	tiff = open_output(directory,"unsupported-first-rgba16.tif","wl");
	page(tiff,2,1,8,4,PHOTOMETRIC_SEPARATED,0,COMPRESSION_NONE,0,6);
	page(tiff,2,2,16,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_NONE,0,16);
	TIFFClose(tiff);
	tiff = open_output(directory,"associated-first-rgba8.tif","wb");
	page(tiff,2,1,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_ASSOCALPHA,COMPRESSION_NONE,0,6);
	page(tiff,3,2,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_NONE,0,8);
	TIFFClose(tiff);
	tiff = open_output(directory,"gray-endian-packbits.tif","wb");
	page(tiff,5,3,8,1,PHOTOMETRIC_MINISWHITE,0,COMPRESSION_PACKBITS,0,4);
	page(tiff,3,2,16,2,PHOTOMETRIC_MINISBLACK,EXTRASAMPLE_UNASSALPHA,COMPRESSION_PACKBITS,0,9);
	TIFFClose(tiff);
	tiff = open_output(directory,"gray1-le-fax4.tif","wl");
	page(tiff,17,3,1,1,PHOTOMETRIC_MINISWHITE,0,COMPRESSION_CCITTFAX4,0,1);
	TIFFClose(tiff);
	tiff = open_output(directory,"bigtiff-unqualified.tif","w8l");
	page(tiff,3,2,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_NONE,0,8);
	TIFFClose(tiff);
	tiff = open_output(directory,"float-first-rgba8.tif","wl");
	float_page(tiff);
	page(tiff,3,2,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_NONE,0,8);
	TIFFClose(tiff);
	tiff = open_output(directory,"icc-srgb-rgba8.tif","wl");
	srgb_profile(tiff);
	page(tiff,3,2,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_NONE,0,8);
	TIFFClose(tiff);
	tiff = open_output(directory,"jpeg-gray-rgb8.tif","wl");
	page(tiff,3,2,8,1,PHOTOMETRIC_MINISBLACK,0,COMPRESSION_JPEG,0,100);
	page(tiff,3,2,8,3,PHOTOMETRIC_RGB,0,COMPRESSION_JPEG,0,100);
	TIFFClose(tiff);
	tiff = open_output(directory,"gray-alpha-8-16-both-photos.tif","wb");
	page(tiff,3,2,8,2,PHOTOMETRIC_MINISWHITE,EXTRASAMPLE_UNASSALPHA,COMPRESSION_PACKBITS,0,9);
	page(tiff,3,2,8,2,PHOTOMETRIC_MINISBLACK,EXTRASAMPLE_UNASSALPHA,COMPRESSION_PACKBITS,0,9);
	page(tiff,3,2,16,2,PHOTOMETRIC_MINISWHITE,EXTRASAMPLE_UNASSALPHA,COMPRESSION_LZW,0,9);
	page(tiff,3,2,16,2,PHOTOMETRIC_MINISBLACK,EXTRASAMPLE_UNASSALPHA,COMPRESSION_LZW,0,9);
	TIFFClose(tiff);
	tiff = open_output(directory,"icc-srgb-larger-rgba8.tif","wl");
	srgb_profile(tiff);
	page(tiff,80,64,8,4,PHOTOMETRIC_RGB,EXTRASAMPLE_UNASSALPHA,COMPRESSION_NONE,0,8);
	TIFFClose(tiff);
}
static uint32_t *read_samples(TIFF *tiff, uint32_t width, uint32_t height, uint16_t bits, uint16_t channels) {
	uint32_t *samples = calloc((size_t)width * height * channels, sizeof(uint32_t));
	if (!samples) fail("reference samples allocation");
	if (TIFFIsTiled(tiff)) {
		uint32_t tw=0,th=0; TIFFGetField(tiff,TIFFTAG_TILEWIDTH,&tw); TIFFGetField(tiff,TIFFTAG_TILELENGTH,&th);
		void *tile=calloc(1,(size_t)TIFFTileSize(tiff)); if (!tile) fail("reference tile allocation");
		for(uint32_t y=0;y<height;y+=th) for(uint32_t x=0;x<width;x+=tw) {
			if(TIFFReadTile(tiff,tile,x,y,0,0)<0) fail("reference raw tile read");
			for(uint32_t dy=0;dy<th && y+dy<height;dy++) for(uint32_t dx=0;dx<tw && x+dx<width;dx++) for(uint16_t c=0;c<channels;c++) {
				size_t from=((size_t)dy*tw+dx)*channels+c,to=((size_t)(y+dy)*width+x+dx)*channels+c;
				samples[to]=bits==8 ? ((uint8_t *)tile)[from] : ((uint16_t *)tile)[from];
			}
		}
		free(tile);
	} else {
		void *row=calloc(1,(size_t)TIFFScanlineSize(tiff)); if(!row) fail("reference row allocation");
		for(uint32_t y=0;y<height;y++) {
			if(TIFFReadScanline(tiff,row,y,0)<0) fail("reference raw scanline read");
			for(uint32_t x=0;x<width;x++) for(uint16_t c=0;c<channels;c++) {
				size_t from=(size_t)x*channels+c,to=((size_t)y*width+x)*channels+c;
				samples[to]=bits==1 ? ((((uint8_t *)row)[x/8]>>(7-x%8))&1) : bits==8 ? ((uint8_t *)row)[from] : bits==32 ? ((uint32_t *)row)[from] : ((uint16_t *)row)[from];
			}
		}
		free(row);
	}
	return samples;
}
static void inspect(const char *path) {
	TIFF *tiff=TIFFOpen(path,"r"); if(!tiff) fail("reference TIFF open");
	int first=1; printf("[");
	do {
		uint32_t w=0,h=0; uint16_t bits=0,channels=0,photo=0,compression=0,format=0,nextra=0,*extra=NULL;
		TIFFGetField(tiff,TIFFTAG_IMAGEWIDTH,&w);TIFFGetField(tiff,TIFFTAG_IMAGELENGTH,&h);
		TIFFGetFieldDefaulted(tiff,TIFFTAG_BITSPERSAMPLE,&bits);TIFFGetFieldDefaulted(tiff,TIFFTAG_SAMPLESPERPIXEL,&channels);
		TIFFGetField(tiff,TIFFTAG_PHOTOMETRIC,&photo);TIFFGetFieldDefaulted(tiff,TIFFTAG_COMPRESSION,&compression);
		TIFFGetFieldDefaulted(tiff,TIFFTAG_SAMPLEFORMAT,&format);TIFFGetField(tiff,TIFFTAG_EXTRASAMPLES,&nextra,&extra);
		uint32_t *samples=read_samples(tiff,w,h,bits,channels);
		printf("%s{\"width\":%u,\"height\":%u,\"bits\":%u,\"channels\":%u,\"photometric\":%u,\"compression\":%u,\"sample_format\":%u,\"extra\":%u,\"tiled\":%s,\"samples\":[",first?"":",",w,h,bits,channels,photo,compression,format,nextra?extra[0]:0,TIFFIsTiled(tiff)?"true":"false");
		for(size_t i=0;i<(size_t)w*h*channels;i++) printf("%s%u",i?",":"",samples[i]);
		printf("],\"icc_hex\":\"");
		uint32_t icc_bytes=0;void *icc=NULL;
		if(TIFFGetField(tiff,TIFFTAG_ICCPROFILE,&icc_bytes,&icc)) for(uint32_t i=0;i<icc_bytes;i++) printf("%02x",((unsigned char *)icc)[i]);
		printf("\"}");free(samples);first=0;
	} while(TIFFReadDirectory(tiff));
	printf("]\n");TIFFClose(tiff);
}
int main(int argc,char **argv) {
	if(argc!=3) fail("reference generate <directory> | inspect <file>");
	if(strcmp(argv[1],"generate")==0) generate(argv[2]);
	else if(strcmp(argv[1],"inspect")==0) inspect(argv[2]);
	else fail("unknown reference operation");
	return 0;
}
