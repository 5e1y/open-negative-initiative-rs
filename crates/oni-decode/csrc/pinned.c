// LibRaw driven with every automatic knob pinned, so a file decodes the same way on every machine.
#include <libraw.h>
#include <string.h>

typedef struct {
    int width, height;
    float pre_mul[4];
    float rgb_cam[3][4];
    unsigned short *pixels;
    libraw_processed_image_t *image;
    libraw_data_t *raw;
} oni_decoded;

int oni_decode(const char *path, float exposure_gain, oni_decoded *out)
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
    int rc = libraw_open_file(lr, path);
    if (rc == LIBRAW_SUCCESS) rc = libraw_unpack(lr);
    if (rc != LIBRAW_SUCCESS) { libraw_close(lr); return rc; }
    // The camera's daylight balance, fixed per body: never the as-shot value, never measured.
    for (int c = 0; c < 4; c++) p->user_mul[c] = lr->color.pre_mul[c] > 0 ? lr->color.pre_mul[c] : lr->color.pre_mul[1];
    rc = libraw_dcraw_process(lr);
    if (rc != LIBRAW_SUCCESS) { libraw_close(lr); return rc; }
    int err = 0;
    libraw_processed_image_t *img = libraw_dcraw_make_mem_image(lr, &err);
    if (!img || img->colors != 3 || img->bits != 16) { if (img) libraw_dcraw_clear_mem(img); libraw_close(lr); return err ? err : -2; }
    out->width = img->width;
    out->height = img->height;
    for (int c = 0; c < 4; c++) out->pre_mul[c] = p->user_mul[c];
    memcpy(out->rgb_cam, lr->color.rgb_cam, sizeof out->rgb_cam);
    out->pixels = (unsigned short *)img->data;
    out->image = img;
    out->raw = lr;
    return 0;
}

void oni_release(oni_decoded *d)
{
    if (d->image) libraw_dcraw_clear_mem(d->image);
    if (d->raw) libraw_close(d->raw);
    memset(d, 0, sizeof *d);
}
