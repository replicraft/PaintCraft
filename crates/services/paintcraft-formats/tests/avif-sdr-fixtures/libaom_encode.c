/* PaintCraft-owned fixture encoder: explicit planar samples and AV1 metadata. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "aom/aom_encoder.h"
#include "aom/aomcx.h"
int main(int argc, char **argv) {
    if (argc != 8) return 2;
    unsigned width = (unsigned)atoi(argv[2]), height = (unsigned)atoi(argv[3]);
    int mono = !strcmp(argv[4], "mono"), subsampled = strcmp(argv[4], "444") != 0;
    int position = atoi(argv[5]), full = atoi(argv[6]);
    if (!width || !height || width > 64 || height > 64 || position < 0 || position > 2 || (full != 0 && full != 1)) return 3;
    aom_codec_enc_cfg_t config;
    if (aom_codec_enc_config_default(aom_codec_av1_cx(), &config, AOM_USAGE_ALL_INTRA)) return 4;
    config.g_w = width; config.g_h = height;
    config.g_threads = 1; config.g_limit = 1; config.g_lag_in_frames = 0;
    config.g_profile = subsampled ? 0 : 1;
    config.g_bit_depth = AOM_BITS_8; config.g_input_bit_depth = 8;
    config.g_timebase.num = 1; config.g_timebase.den = 1;
    config.monochrome = mono; config.rc_end_usage = AOM_Q;
    config.rc_min_quantizer = config.rc_max_quantizer = 0;
    aom_codec_ctx_t codec = {0};
    if (aom_codec_enc_init(&codec, aom_codec_av1_cx(), &config, 0)) return 5;
#define CONTROL(name, value) do { if (aom_codec_control(&codec, name, value)) { fprintf(stderr, "%s: %s\n", #name, aom_codec_error(&codec)); return 6; } } while (0)
    CONTROL(AOME_SET_CPUUSED, 8);
    CONTROL(AV1E_SET_LOSSLESS, 1u);
    CONTROL(AV1E_SET_FORCE_VIDEO_MODE, 0u);
    CONTROL(AV1E_SET_COLOR_PRIMARIES, mono ? 2 : 1);
    CONTROL(AV1E_SET_TRANSFER_CHARACTERISTICS, mono ? 2 : 13);
    CONTROL(AV1E_SET_MATRIX_COEFFICIENTS, mono ? 2 : 1);
    CONTROL(AV1E_SET_COLOR_RANGE, full);
    CONTROL(AV1E_SET_CHROMA_SAMPLE_POSITION, position);
    aom_image_t image;
    if (!aom_img_alloc(&image, subsampled ? AOM_IMG_FMT_I420 : AOM_IMG_FMT_I444, width, height, 1)) return 7;
    image.monochrome = mono;
    image.cp = mono ? AOM_CICP_CP_UNSPECIFIED : AOM_CICP_CP_BT_709;
    image.tc = mono ? AOM_CICP_TC_UNSPECIFIED : AOM_CICP_TC_SRGB;
    image.mc = mono ? AOM_CICP_MC_UNSPECIFIED : AOM_CICP_MC_BT_709;
    image.range = full ? AOM_CR_FULL_RANGE : AOM_CR_STUDIO_RANGE;
    image.csp = (aom_chroma_sample_position_t)position;
    FILE *input = fopen(argv[7], "rb");
    if (!input) return 8;
    for (unsigned plane = 0; plane < 3; plane++) {
        unsigned sx = plane && subsampled, sy = sx;
        unsigned w = (width + ((1u << sx) - 1)) >> sx;
        unsigned h = (height + ((1u << sy) - 1)) >> sy;
        for (unsigned y = 0; y < h; y++) {
            unsigned char *row = image.planes[plane] + y * image.stride[plane];
            if (mono && plane) memset(row, 128, w);
            else if (fread(row, 1, w, input) != w) return 9;
        }
    }
    if (fgetc(input) != EOF) return 10;
    fclose(input);
    FILE *output = fopen(argv[1], "wb"); if (!output) return 11;
    unsigned frames = 0;
    for (unsigned pass = 0; pass < 2; pass++) {
        if (aom_codec_encode(&codec, pass ? NULL : &image, 0, 1, 0)) { fprintf(stderr, "%s\n", aom_codec_error(&codec)); return 12; }
        aom_codec_iter_t iter = NULL;
        const aom_codec_cx_pkt_t *packet;
        while ((packet = aom_codec_get_cx_data(&codec, &iter))) {
            if (packet->kind != AOM_CODEC_CX_FRAME_PKT) continue;
            frames++;
            if (packet->data.frame.sz > 65536 || fwrite(packet->data.frame.buf, 1, packet->data.frame.sz, output) != packet->data.frame.sz) return 13;
        }
    }
    if (fclose(output) || frames != 1) return 14;
    printf("%s\n", aom_codec_version_str());
    aom_img_free(&image); aom_codec_destroy(&codec);
    return 0;
}
