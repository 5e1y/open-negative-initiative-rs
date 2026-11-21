"""spektrafilm's chain on Portra 800 + Portra Endura as a per-pixel transform, with its taps."""
import numpy as np
from spektrafilm.runtime.params_builder import digest_params, init_params
from spektrafilm.runtime.pipeline import SimulationPipeline

def pipeline(scan=False, print_exposure=1.0, **enlarger):
    p = init_params(film_profile="kodak_portra_800", print_profile="kodak_portra_endura")
    p.debug.lut_mode = True
    p.io.input_color_space = "ITU-R BT.2020"
    p.io.input_cctf_decoding = False
    p.io.output_color_space = "ITU-R BT.2020"
    p.io.output_cctf_encoding = False
    p.io.scan_film = scan
    d = digest_params(p)
    d.enlarger.print_exposure = print_exposure
    for k, v in enlarger.items():
        setattr(d.enlarger, k, v)
    return SimulationPipeline(d)

def run(pipe, img, inject=None, collect=None):
    img = np.asarray(img, dtype=np.float64)
    shape = img.shape
    out = pipe.process(img.reshape(1, -1, 3), inject=inject, collect=collect)
    return np.asarray(out, dtype=np.float64).reshape(shape)
