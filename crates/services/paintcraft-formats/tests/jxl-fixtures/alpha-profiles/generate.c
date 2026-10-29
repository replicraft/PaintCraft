#include <jxl/encode.h>
#include <jxl/decode.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#define OK(expr) do { if ((expr) != JXL_ENC_SUCCESS) { fprintf(stderr, "encoder failed line %d\n", __LINE__); return 1; } } while (0)
int main(int argc, char **argv) {
    if (argc != 7) return 2;
    const unsigned color = atoi(argv[2]), alpha = atoi(argv[3]);
    const unsigned exponent = atoi(argv[4]), shift = atoi(argv[5]), named = atoi(argv[6]);
    JxlEncoder *enc = JxlEncoderCreate(NULL);
    JxlBasicInfo basic; JxlEncoderInitBasicInfo(&basic);
    basic.xsize = 2; basic.ysize = 2; basic.bits_per_sample = color;
    basic.num_color_channels = 3; basic.num_extra_channels = 1;
    basic.alpha_bits = alpha; basic.alpha_exponent_bits = exponent;
    basic.uses_original_profile = JXL_TRUE;
    OK(JxlEncoderSetBasicInfo(enc, &basic));
    JxlColorEncoding color_encoding; JxlColorEncodingSetToSRGB(&color_encoding, JXL_FALSE);
    OK(JxlEncoderSetColorEncoding(enc, &color_encoding));
    JxlExtraChannelInfo channel; JxlEncoderInitExtraChannelInfo(JXL_CHANNEL_ALPHA, &channel);
    channel.bits_per_sample = alpha; channel.exponent_bits_per_sample = exponent;
    channel.dim_shift = shift;
    OK(JxlEncoderSetExtraChannelInfo(enc, 0, &channel));
    if (named) OK(JxlEncoderSetExtraChannelName(enc, 0, "coverage", 8));
    JxlEncoderFrameSettings *settings = JxlEncoderFrameSettingsCreate(enc, NULL);
    OK(JxlEncoderSetFrameLossless(settings, JXL_TRUE));
    const uint16_t pixels[] = {0,257,65535,0, 257,32767,32768,257,
                              65535,1024,2048,32768, 4096,8192,16384,65535};
    JxlPixelFormat format = {4,JXL_TYPE_UINT16,JXL_NATIVE_ENDIAN,0};
    if (shift || exponent) {
        const uint16_t rgb[] = {0,257,65535,257,32767,32768,65535,1024,2048,4096,8192,16384};
        JxlPixelFormat rgbformat = {3,JXL_TYPE_UINT16,JXL_NATIVE_ENDIAN,0};
        OK(JxlEncoderAddImageFrame(settings, &rgbformat, rgb, sizeof(rgb)));
        if (exponent) {
            const float alpha_values[] = {0.0f,0.125f,0.5f,1.0f};
            JxlPixelFormat aformat = {1,JXL_TYPE_FLOAT,JXL_NATIVE_ENDIAN,0};
            OK(JxlEncoderSetExtraChannelBuffer(settings, &aformat, alpha_values, sizeof(alpha_values), 0));
        } else {
            const uint16_t alpha_values[] = {32768,32768,32768,32768};
            JxlPixelFormat aformat = {1,JXL_TYPE_UINT16,JXL_NATIVE_ENDIAN,0};
            OK(JxlEncoderSetExtraChannelBuffer(settings, &aformat, alpha_values, sizeof(alpha_values), 0));
        }
    } else {
        OK(JxlEncoderAddImageFrame(settings, &format, pixels, sizeof(pixels)));
    }
    JxlEncoderCloseInput(enc);
    uint8_t output[65536]; uint8_t *next = output; size_t available = sizeof(output);
    OK(JxlEncoderProcessOutput(enc, &next, &available));
    FILE *file = fopen(argv[1], "wb"); if (!file) return 3;
    const size_t length = sizeof(output)-available;
    if (fwrite(output,1,length,file) != length || fclose(file)) return 4;
    JxlEncoderDestroy(enc);
    JxlDecoder *dec = JxlDecoderCreate(NULL);
    JxlDecoderSubscribeEvents(dec, JXL_DEC_BASIC_INFO);
    JxlDecoderSetInput(dec, output, length); JxlDecoderCloseInput(dec);
    if (JxlDecoderProcessInput(dec) != JXL_DEC_BASIC_INFO) return 5;
    JxlBasicInfo result; JxlExtraChannelInfo actual;
    if (JxlDecoderGetBasicInfo(dec,&result) != JXL_DEC_SUCCESS ||
        JxlDecoderGetExtraChannelInfo(dec,0,&actual) != JXL_DEC_SUCCESS) return 6;
    printf("color=%u alpha=%u exponent=%u shift=%u name_length=%u bytes=%zu\n",
        result.bits_per_sample,actual.bits_per_sample,actual.exponent_bits_per_sample,
        actual.dim_shift,actual.name_length,length);
    JxlDecoderDestroy(dec);
    return result.bits_per_sample != color || actual.bits_per_sample != alpha ||
           actual.exponent_bits_per_sample != exponent || actual.dim_shift != shift ||
           (actual.name_length != (named ? 8u : 0u));
}
