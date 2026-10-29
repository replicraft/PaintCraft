/* Independent raw libtiff check. TIFFReadScanline leaves orientation and
 * straight alpha untouched; no RGBA convenience routines participate. */
#include <tiffio.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static _Noreturn void fail(const char *message) {
    fprintf(stderr, "%s\n", message);
    exit(1);
}

int main(int argc, char **argv) {
    if (argc != 2) fail("expected one TIFF filename");
    TIFF *tiff = TIFFOpen(argv[1], "r");
    if (!tiff) fail("could not open fixture");
    printf("[");
    int page = 0;
    do {
        uint32_t width, height;
        uint16_t orientation, bits, channels, photo;
        if (!TIFFGetField(tiff, TIFFTAG_IMAGEWIDTH, &width) ||
            !TIFFGetField(tiff, TIFFTAG_IMAGELENGTH, &height) ||
            !TIFFGetField(tiff, TIFFTAG_ORIENTATION, &orientation) ||
            !TIFFGetField(tiff, TIFFTAG_BITSPERSAMPLE, &bits) ||
            !TIFFGetField(tiff, TIFFTAG_SAMPLESPERPIXEL, &channels) ||
            !TIFFGetField(tiff, TIFFTAG_PHOTOMETRIC, &photo)) fail("missing tag");
        if (width != 3 || height != 2 || (bits != 8 && bits != 16) ||
            (channels != 2 && channels != 4)) fail("unexpected fixture layout");
        tmsize_t stride = TIFFScanlineSize(tiff);
        void *row = _TIFFmalloc(stride);
        if (!row) fail("allocation refused");
        printf("%s{\"width\":%u,\"height\":%u,\"orientation\":%u,"
               "\"bits\":%u,\"channels\":%u,\"photometric\":%u,\"samples\":[",
               page++ ? "," : "", width, height, orientation, bits, channels, photo);
        for (uint32_t y = 0; y < height; ++y) {
            if (TIFFReadScanline(tiff, row, y, 0) < 0) fail("scanline failed");
            for (uint32_t x = 0; x < width * channels; ++x) {
                uint32_t sample = bits == 8 ? ((uint8_t *)row)[x] : ((uint16_t *)row)[x];
                printf("%s%u", (x || y) ? "," : "", sample);
            }
        }
        printf("]}");
        _TIFFfree(row);
    } while (TIFFReadDirectory(tiff));
    printf("]\n");
    TIFFClose(tiff);
    return 0;
}
