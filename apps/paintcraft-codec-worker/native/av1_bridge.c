/* PaintCraft-owned narrow ABI. All dav1d allocations use patched mem.h. */
#include <errno.h>
#include <stdint.h>
#include <string.h>
#include "dav1d/dav1d.h"

struct PaintcraftAv1Info {
    uint32_t width, height, layout, depth, profile, level, max_width, max_height;
    uint32_t primaries, transfer, matrix, full_range;
    uint32_t chroma_position, still_picture, reduced_header;
};
int paintcraft_av1_decode(const uint8_t *, size_t, uint8_t *, size_t,
                         struct PaintcraftAv1Info *);
/* output is caller-owned storage for three full-sized 8-bit planes. Chroma
 * planes are tightly packed at their declared subsampled sizes in each slot. */
int paintcraft_av1_decode(const uint8_t *input, size_t length,
                         uint8_t *output, size_t output_length,
                         struct PaintcraftAv1Info *info) {
    if (!output_length || output_length % 3 || output_length / 3 > 4194304) {
        return DAV1D_ERR(EINVAL);
    }
    Dav1dSettings settings;
    dav1d_default_settings(&settings);
    settings.n_threads = 1;
    settings.max_frame_delay = 1;
    settings.frame_size_limit = (unsigned)(output_length / 3);
    settings.strict_std_compliance = 1;
    settings.all_layers = 1;
    settings.logger.callback = NULL;
    Dav1dContext *context = NULL;
    Dav1dData data = {0};
    Dav1dPicture picture = {0};
    int have_picture = 0;
    int result = dav1d_open(&context, &settings);
    if (result < 0) return result;
    uint8_t *owned = dav1d_data_create(&data, length);
    if (!owned) {result = DAV1D_ERR(ENOMEM); goto done;}
    memcpy(owned, input, length);
    unsigned calls;
    for (calls = 0; calls < 1024; calls++) {
        if (data.sz) {
            result = dav1d_send_data(context, &data);
            if (result < 0 && result != DAV1D_ERR(EAGAIN)) goto done;
        }
        Dav1dPicture next = {0};
        result = dav1d_get_picture(context, &next);
        if (result == 0) {
            if (have_picture) {
                dav1d_picture_unref(&next);
                result = DAV1D_ERR(EINVAL);
                goto done;
            }
            picture = next;
            have_picture = 1;
        } else if (result == DAV1D_ERR(EAGAIN)) {
            if (!data.sz) break;
        } else goto done;
    }
    if (calls == 1024 || !have_picture || picture.p.w <= 0 || picture.p.h <= 0 ||
        picture.p.bpc != 8 || !picture.seq_hdr ||
        picture.seq_hdr->num_operating_points != 1 ||
        (!picture.seq_hdr->reduced_still_picture_header &&
         picture.seq_hdr->operating_points[0].major_level < 2) ||
        picture.seq_hdr->operating_points[0].minor_level > 3) {
        result = DAV1D_ERR(EINVAL); goto done;
    }
    const size_t width = picture.p.w, height = picture.p.h;
    if (width > 65536 || height > 65536 || width * height > 4194304 ||
        output_length < width * height * 3) {
        result = DAV1D_ERR(EINVAL); goto done;
    }
    const size_t area = width * height;
    for (unsigned plane = 0; plane < 3; plane++) {
        if (plane && picture.p.layout == DAV1D_PIXEL_LAYOUT_I400) break;
        const int sx = plane && picture.p.layout != DAV1D_PIXEL_LAYOUT_I444;
        const int sy = plane && picture.p.layout == DAV1D_PIXEL_LAYOUT_I420;
        const size_t pw = (width + sx) >> sx, ph = (height + sy) >> sy;
        const ptrdiff_t stride = picture.stride[plane != 0];
        if (!picture.data[plane] || stride < 0 || (size_t)stride < pw) {
            result = DAV1D_ERR(EINVAL); goto done;
        }
        for (size_t y = 0; y < ph; y++) {
            memcpy(output + plane * area + y * pw,
                   (const uint8_t *)picture.data[plane] + y * stride, pw);
        }
    }
    *info = (struct PaintcraftAv1Info){
        .width = width, .height = height,
        .layout = picture.p.layout, .depth = picture.p.bpc,
        .profile = picture.seq_hdr->profile,
        /* dav1d stores the raw upper three bits for reduced-still headers,
         * while non-reduced headers store nominal major level (raw + 2). */
        .level = (picture.seq_hdr->operating_points[0].major_level -
                  (picture.seq_hdr->reduced_still_picture_header ? 0 : 2)) * 4 +
                 picture.seq_hdr->operating_points[0].minor_level,
        .max_width = picture.seq_hdr->max_width,
        .max_height = picture.seq_hdr->max_height,
        .primaries = picture.seq_hdr->pri, .transfer = picture.seq_hdr->trc,
        .matrix = picture.seq_hdr->mtrx, .full_range = picture.seq_hdr->color_range,
        .chroma_position = picture.seq_hdr->chr,
        .still_picture = picture.seq_hdr->still_picture,
        .reduced_header = picture.seq_hdr->reduced_still_picture_header,
    };
    result = 0;
done:
    dav1d_data_unref(&data);
    if (have_picture) dav1d_picture_unref(&picture);
    dav1d_close(&context);
    return result;
}
