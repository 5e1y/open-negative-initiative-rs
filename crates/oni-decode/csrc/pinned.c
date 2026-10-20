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
    // 1 when LibRaw handed out half the sensor's sides, each pixel one 2x2 block, no demosaic.
    int half;
    // The maker's default crop, kept once LibRaw's own buffers are let go.
    int inset[4];
    // The file's orientation, LibRaw's `flip`; the pixels come out unturned and Rust turns them.
    int flip;
    // The camera as `oni_identify` names it, which keys its rendition matrix.
    char make[64], model[64];
} oni_decoded;

void oni_release(oni_decoded *d)
{
    if (d->image) libraw_dcraw_clear_mem(d->image);
    if (d->raw) libraw_close(d->raw);
    memset(d, 0, sizeof *d);
}

// Opens and identifies: the colour metadata is read before any multiplier is chosen. A copy no
// longer than `min_side` (0: the whole frame) is read at half size when the sensor allows it.
int oni_open(const char *path, float exposure_gain, int min_side, oni_decoded *out)
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
    // Left at 0.75, LibRaw lowers the white level to a bright frame's own peak: a choice per frame.
    p->adjust_maximum_thr = 0.0f;
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
    int longest = lr->sizes.width > lr->sizes.height ? lr->sizes.width : lr->sizes.height;
    if (min_side > 0 && longest >= 2 * min_side) {
        p->half_size = 1;
        out->half = 1;
    }
    out->flip = lr->sizes.flip;
    out->colors = lr->idata.colors;
    out->dng = lr->idata.dng_version != 0;
    memcpy(out->cam_xyz, lr->color.cam_xyz, sizeof out->cam_xyz);
    out->baseline_exposure = lr->color.dng_levels.baseline_exposure;
    strncpy(out->make, lr->idata.make, 63);
    strncpy(out->model, lr->idata.model, 63);
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
    const libraw_raw_inset_crop_t *c = &lr->sizes.raw_inset_crops[0];
    d->inset[0] = c->cleft;
    d->inset[1] = c->ctop;
    d->inset[2] = c->cwidth;
    d->inset[3] = c->cheight;
    // The raw and working buffers, 245 MB at 24 MP, go before the caller's float copy is made.
    libraw_recycle(lr);
    return 0;
}

// The maker's default crop inside the visible area, as LibRaw reads it; zero sizes when none.
void oni_inset(const oni_decoded *d, int out[4])
{
    memcpy(out, d->inset, sizeof d->inset);
}

// The file's embedded preview as LibRaw hands it, never demosaiced: a JPEG's bytes or an 8-bit
// bitmap, unturned, with the frame's flip to turn it by.
typedef struct {
    int format, width, height, colors, bits, flip;
    unsigned int size;
    unsigned char *data;
    libraw_processed_image_t *image;
} oni_thumb;

void oni_thumb_release(oni_thumb *t)
{
    if (t->image) libraw_dcraw_clear_mem(t->image);
    memset(t, 0, sizeof *t);
}

int oni_thumbnail(const char *path, oni_thumb *out)
{
    memset(out, 0, sizeof *out);
    libraw_data_t *lr = libraw_init(0);
    if (!lr) return -1;
    int rc = libraw_open_file(lr, path);
    if (rc == LIBRAW_SUCCESS) rc = libraw_unpack_thumb(lr);
    if (rc != LIBRAW_SUCCESS) { libraw_close(lr); return rc; }
    int err = 0;
    libraw_processed_image_t *img = libraw_dcraw_make_mem_thumb(lr, &err);
    out->flip = lr->sizes.flip;
    libraw_close(lr);
    if (!img) return err ? err : -2;
    out->image = img;
    out->format = img->type;
    out->width = img->width;
    out->height = img->height;
    out->colors = img->colors;
    out->bits = img->bits;
    out->size = img->data_size;
    out->data = img->data;
    return 0;
}

// The maker and model LibRaw names from the file's metadata, and whether it is a DNG; nothing decoded.
int oni_identify(const char *path, char make[64], char model[64], int *dng)
{
    libraw_data_t *lr = libraw_init(0);
    if (!lr) return -1;
    int rc = libraw_open_file(lr, path);
    if (rc == LIBRAW_SUCCESS) {
        strncpy(make, lr->idata.make, 63);
        make[63] = 0;
        strncpy(model, lr->idata.model, 63);
        model[63] = 0;
        *dng = lr->idata.dng_version != 0;
    }
    libraw_close(lr);
    return rc;
}
