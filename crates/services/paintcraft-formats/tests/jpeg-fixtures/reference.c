/* Authored sample arrays and libjpeg-turbo raw component oracle, qualification only. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <jpeglib.h>

static void fail(const char *message) { fprintf(stderr, "%s\n", message); exit(1); }
static void write_fixture(const char *path, const char *kind) {
    struct jpeg_compress_struct c;
    struct jpeg_error_mgr e;
    FILE *file = fopen(path, "wb");
    if (!file) fail("cannot write fixture");
    c.err = jpeg_std_error(&e);
    jpeg_create_compress(&c);
    jpeg_stdio_dest(&c, file);
    c.image_width = 24;
    c.image_height = 16;
    int gray = !strcmp(kind, "gray");
    int rgb = !strcmp(kind, "rgb") || !strcmp(kind, "progressive") || !strcmp(kind, "rgb-numbered");
    int cmyk = !strcmp(kind, "adobe-cmyk") || !strcmp(kind, "unmarked-cmyk") || !strcmp(kind, "adobe-ycck") || !strcmp(kind, "adobe-ycck-color");
    c.input_components = gray ? 1 : cmyk ? 4 : 3;
    c.in_color_space = gray ? JCS_GRAYSCALE : cmyk ? JCS_CMYK : JCS_RGB;
    jpeg_set_defaults(&c);
    if (rgb) jpeg_set_colorspace(&c, JCS_RGB);
    if (!strcmp(kind, "adobe-ycck") || !strcmp(kind, "adobe-ycck-color")) jpeg_set_colorspace(&c, JCS_YCCK);
    if (!strcmp(kind, "rgb-numbered")) {
        for (int i = 0; i < 3; i++) c.comp_info[i].component_id = i + 1;
    }
    if (!strcmp(kind, "unmarked-cmyk")) c.write_Adobe_marker = FALSE;
    for (int i = 0; i < c.num_components; i++) {
        c.comp_info[i].h_samp_factor = 1;
        c.comp_info[i].v_samp_factor = 1;
    }
    jpeg_set_quality(&c, 100, TRUE);
    if (!strcmp(kind, "progressive")) jpeg_simple_progression(&c);
    jpeg_start_compress(&c, TRUE);
    unsigned char row[24 * 4];
    static const unsigned char colors[6][3] = {
        {0,32,64},{96,128,160},{192,224,255},{255,0,128},{64,192,32},{160,96,224}
    };
    static const unsigned char ink[6][4] = {
        {0,0,0,255},{255,0,64,128},{129,255,1,255},
        {1,128,254,1},{255,255,255,255},{80,160,240,0}
    };
    while (c.next_scanline < c.image_height) {
        for (unsigned x = 0; x < c.image_width; x++) {
            unsigned block = (c.next_scanline / 8) * 3 + x / 8;
            unsigned char g = (unsigned char)(block * 40);
            for (int channel = 0; channel < c.input_components; channel++) {
                unsigned char sample = gray ? g : rgb ? colors[block][channel] : g;
                if (cmyk) {
                    /* Equal CMY gives exact neutral YCCK chroma; K is independent. */
                    sample = !strcmp(kind, "adobe-ycck")
                        ? (channel == 3 ? (unsigned char)(block % 2 ? 128 : 255) : g)
                        : ink[block][channel];
                    if (!strcmp(kind, "unmarked-cmyk")) sample = 255 - sample;
                }
                row[x * (unsigned)c.input_components + (unsigned)channel] = sample;
            }
        }
        JSAMPROW pointer = row;
        if (jpeg_write_scanlines(&c, &pointer, 1) != 1) fail("write scanline failed");
    }
    jpeg_finish_compress(&c);
    jpeg_destroy_compress(&c);
    fclose(file);
}
static void read_fixture(const char *path) {
    struct jpeg_decompress_struct d;
    struct jpeg_error_mgr e;
    FILE *file = fopen(path, "rb");
    if (!file) fail("cannot read fixture");
    d.err = jpeg_std_error(&e);
    jpeg_create_decompress(&d);
    jpeg_stdio_src(&d, file);
    if (jpeg_read_header(&d, TRUE) != JPEG_HEADER_OK) fail("bad JPEG header");
    int original = d.jpeg_color_space;
    int four = d.num_components == 4;
    int adobe = d.saw_Adobe_marker;
    int transform = d.Adobe_transform;
    d.out_color_space = four ? JCS_CMYK : d.num_components == 1 ? JCS_GRAYSCALE : JCS_RGB;
    d.dct_method = JDCT_ISLOW;
    d.do_fancy_upsampling = TRUE;
    jpeg_start_decompress(&d);
    size_t size = (size_t)d.output_width * d.output_height * d.output_components;
    unsigned char *samples = malloc(size);
    if (!samples) fail("raw sample allocation failed");
    while (d.output_scanline < d.output_height) {
        JSAMPROW row = samples + (size_t)d.output_scanline * d.output_width * d.output_components;
        if (jpeg_read_scanlines(&d, &row, 1) != 1) fail("read scanline failed");
    }
    printf("{\"width\":%u,\"height\":%u,\"channels\":%d,\"original_jpeg_color_space\":%d,\"adobe\":%s,\"transform\":%d,\"raw_samples\":[", d.output_width, d.output_height, d.output_components, original, adobe ? "true" : "false", transform);
    for (size_t i = 0; i < size; i++) printf("%s%u", i ? "," : "", samples[i]);
    printf("]}\n");
    free(samples);
    jpeg_finish_decompress(&d);
    jpeg_destroy_decompress(&d);
    fclose(file);
}
int main(int argc, char **argv) {
    if (argc == 4 && !strcmp(argv[1], "write")) write_fixture(argv[2], argv[3]);
    else if (argc == 3 && !strcmp(argv[1], "read")) read_fixture(argv[2]);
    else fail("usage: reference write PATH KIND | read PATH");
    return 0;
}
