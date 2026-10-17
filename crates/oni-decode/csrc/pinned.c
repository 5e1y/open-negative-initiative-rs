// LibRaw driven with every automatic knob pinned, so a file decodes the same way on every machine.
#include "libraw/libraw.h"
#include <string.h>

typedef struct {
    int width, height, colors, dng;
    float cam_xyz[4][3];
    // DNG BaselineExposure in stops; -999 when the file carries none.
    float baseline_exposure;
    unsigned short *pixels;
    libraw_processed_image_t *image;
    libraw_data_t *raw;
} oni_decoded;

void oni_release(oni_decoded *d)
{
    if (d->image) libraw_dcraw_clear_mem(d->image);
    if (d->raw) libraw_close(d->raw);
    memset(d, 0, sizeof *d);
}

// Opens and identifies: the colour metadata is read before any multiplier is chosen.
int oni_open(const char *path, float exposure_gain, oni_decoded *out)
{
    memset(out, 0, sizeof *out);
    libraw_data_t *lr = libraw_init(0);
    if (!lr) return -1;
    libraw_output_params_t *p = &lr->params;
    p->use_camera_wb = 0;
    p->use_auto_wb = 0;
    p->use_camera_matrix = 0;
    p->output_color = 0;
    p->output_bps = 16;
    p->gamm[0] = 1.0;
    p->gamm[1] = 1.0;
    p->no_auto_bright = 1;
    // Mode 1 normalises the multipliers on the largest, so no channel is scaled past the ceiling.
    p->highlight = 1;
    p->user_qual = 3;
    p->half_size = 0;
    p->med_passes = 0;
    p->fbdd_noiserd = 0;
    p->user_flip = 0;
    if (exposure_gain != 1.0f) {
        p->exp_correc = 1;
        p->exp_shift = exposure_gain;
        p->exp_preser = 0.0f;
    }
    out->raw = lr;
    int rc = libraw_open_file(lr, path);
    if (rc != LIBRAW_SUCCESS) { oni_release(out); return rc; }
    out->colors = lr->idata.colors;
    out->dng = lr->idata.dng_version != 0;
    memcpy(out->cam_xyz, lr->color.cam_xyz, sizeof out->cam_xyz);
    out->baseline_exposure = lr->color.dng_levels.baseline_exposure;
    return 0;
}

int oni_unpack(oni_decoded *d) { return libraw_unpack(d->raw); }

// Demosaics on the given multipliers, then hands out the camera-space 16-bit image.
int oni_process(oni_decoded *d, const float mul[3])
{
    libraw_data_t *lr = d->raw;
    for (int c = 0; c < 3; c++) lr->params.user_mul[c] = mul[c];
    lr->params.user_mul[3] = mul[1];
    int rc = libraw_dcraw_process(lr);
    if (rc != LIBRAW_SUCCESS) return rc;
    int err = 0;
    libraw_processed_image_t *img = libraw_dcraw_make_mem_image(lr, &err);
    if (!img || img->colors != 3 || img->bits != 16) { if (img) libraw_dcraw_clear_mem(img); return err ? err : -2; }
    d->width = img->width;
    d->height = img->height;
    d->pixels = (unsigned short *)img->data;
    d->image = img;
    return 0;
}
