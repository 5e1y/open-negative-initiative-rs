// LibRaw driven with every automatic knob pinned, so a file decodes the same way on every machine.
#include "libraw/libraw.h"
#include <string.h>

// From stream.cpp: a file reached through a handle the caller opened.
int oni_stream_attach(libraw_data_t *lr, void *stream);
void oni_stream_free(void *stream);

// A file by its name, or through `stream` when there is one; `stream` stays the caller's to free.
static int oni_attach(libraw_data_t *lr, const char *path, void *stream)
{
    return stream ? oni_stream_attach(lr, stream) : libraw_open_file(lr, path);
}

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
    // The handle's stream, freed once LibRaw lets the file go; NULL for a file opened by name.
    void *stream;
    // 1 once LibRaw read past the file's end: a copy cut short, never a picture to show.
    int cut;
} oni_decoded;

// LibRaw tells a data error at the file's end by an offset of -1; others are left to its decoders.
static void oni_data_error(void *data, const char *file, const INT64 offset)
{
    (void)file;
    if (offset < 0) ((oni_decoded *)data)->cut = 1;
}

void oni_release(oni_decoded *d)
{
    if (d->image) libraw_dcraw_clear_mem(d->image);
    if (d->raw) libraw_close(d->raw);
    if (d->stream) oni_stream_free(d->stream);
    memset(d, 0, sizeof *d);
}

// Opens and identifies: the colour metadata is read before any multiplier is chosen. A copy no
// longer than `min_side` (0: the whole frame) is read at half size when the sensor allows it.
// `stream`, when given, is the file's and freed by `oni_release`.
int oni_open(const char *path, void *stream, float exposure_gain, int min_side, oni_decoded *out)
{
    memset(out, 0, sizeof *out);
    out->stream = stream;
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { oni_release(out); return -1; }
    libraw_set_dataerror_handler(lr, oni_data_error, out);
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
    int rc = oni_attach(lr, path, stream);
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

// -5 for a file that ends before its pixels do, whatever LibRaw's own code for it.
int oni_unpack(oni_decoded *d)
{
    int rc = libraw_unpack(d->raw);
    return d->cut ? -5 : rc;
}

// Demosaics on the given multipliers, then hands out the camera-space 16-bit image.
int oni_process(oni_decoded *d, const float mul[3])
{
    libraw_data_t *lr = d->raw;
    // LibRaw halves a mosaic alone (raw2image_start): a LinearRaw DNG or an sRAW comes back whole.
    const libraw_rawdata_t *rd = &lr->rawdata;
    if (!rd->iparams.filters || rd->color3_image || rd->color4_image || rd->float3_image || rd->float4_image) {
        lr->params.half_size = 0;
        d->half = 0;
    }
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
    // LibRaw keeps the crop in the raw's coordinates; the image handed out starts at the margins.
    const libraw_raw_inset_crop_t *c = &lr->sizes.raw_inset_crops[0];
    int left = rd->sizes.left_margin, top = rd->sizes.top_margin;
    if (c->cleft < 0xffff && c->ctop < 0xffff && c->cleft >= left && c->ctop >= top) {
        d->inset[0] = c->cleft - left;
        d->inset[1] = c->ctop - top;
        d->inset[2] = c->cwidth;
        d->inset[3] = c->cheight;
    }
    // The raw and working buffers, 245 MB at 24 MP, go before the caller's float copy is made.
    libraw_recycle(lr);
    if (d->stream) { oni_stream_free(d->stream); d->stream = NULL; }
    return 0;
}

// The maker's default crop in the image handed out, before any halving; zero sizes when none.
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

// `stream`, when given, is freed before the call returns.
int oni_thumbnail(const char *path, void *stream, oni_thumb *out)
{
    memset(out, 0, sizeof *out);
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { if (stream) oni_stream_free(stream); return -1; }
    int rc = oni_attach(lr, path, stream);
    if (rc == LIBRAW_SUCCESS) rc = libraw_unpack_thumb(lr);
    if (rc != LIBRAW_SUCCESS) { libraw_close(lr); if (stream) oni_stream_free(stream); return rc; }
    int err = 0;
    libraw_processed_image_t *img = libraw_dcraw_make_mem_thumb(lr, &err);
    out->flip = lr->sizes.flip;
    libraw_close(lr);
    if (stream) oni_stream_free(stream);
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
// `stream`, when given, is freed before the call returns.
int oni_identify(const char *path, void *stream, char make[64], char model[64], int *dng)
{
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { if (stream) oni_stream_free(stream); return -1; }
    int rc = oni_attach(lr, path, stream);
    if (rc == LIBRAW_SUCCESS) {
        strncpy(make, lr->idata.make, 63);
        make[63] = 0;
        strncpy(model, lr->idata.model, 63);
        model[63] = 0;
        *dng = lr->idata.dng_version != 0;
    }
    libraw_close(lr);
    if (stream) oni_stream_free(stream);
    return rc;
}

// What the file says of its making, as LibRaw reads it from the metadata: nothing is decoded.
typedef struct {
    char make[64], model[64], lens[128], artist[64];
    float iso;
} oni_described;

// `stream`, when given, is freed before the call returns.
int oni_describe(const char *path, void *stream, oni_described *out)
{
    memset(out, 0, sizeof *out);
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { if (stream) oni_stream_free(stream); return -1; }
    int rc = oni_attach(lr, path, stream);
    if (rc == LIBRAW_SUCCESS) {
        strncpy(out->make, lr->idata.make, 63);
        strncpy(out->model, lr->idata.model, 63);
        strncpy(out->lens, lr->lens.Lens, 127);
        strncpy(out->artist, lr->other.artist, 63);
        out->iso = lr->other.iso_speed;
    }
    libraw_close(lr);
    if (stream) oni_stream_free(stream);
    return rc;
}
