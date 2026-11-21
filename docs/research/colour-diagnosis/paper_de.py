"""dE2000 of each chart-chain print (colour_trace's picture) to spektrafilm's Portra 800 + Endura C, at the exposure that
prints a grey as ONI's print does, with and without spektrafilm's output gamut compression; C1 at spektrafilm's own."""
import sys, os, dataclasses
import numpy as np
from io32 import read
import lab, spk
from base import C, C1, PE, CHART, PH, RAMP, P
from spektrafilm.runtime.params_builder import digest_params, init_params
from spektrafilm.runtime.pipeline import SimulationPipeline

def uncompressed(paper="kodak_portra_endura", pe=1.0):
    p = init_params(film_profile="kodak_portra_800", print_profile=paper)
    p.debug.lut_mode = True
    p.io.input_color_space = "ITU-R BT.2020"; p.io.input_cctf_decoding = False
    p.io.output_color_space = "ITU-R BT.2020"; p.io.output_cctf_encoding = False
    p.io.output_gamut_compress = dataclasses.replace(p.io.output_gamut_compress, algorithm="off")
    d = digest_params(p); d.enlarger.print_exposure = pe
    return SimulationPipeline(d)

Cu = spk.run(uncompressed(pe=PE), P)
print(f"PE {PE:.4f}")
refs = {"C": C, "C-uncompressed": Cu, "C1 (PE 1)": C1}
for d in sys.argv[1:]:
    B = read(os.path.join(d, "picture.f32"))[0]
    line = f"{os.path.basename(d):18s}"
    for name, ref in refs.items():
        e = lab.de(B, ref)
        line += f" | {name}: chart {e[CHART].mean():5.2f} (max {e[CHART].max():4.1f}) photo {e[PH].mean():5.2f} greys {e[RAMP].mean():5.2f}"
    print(line)
