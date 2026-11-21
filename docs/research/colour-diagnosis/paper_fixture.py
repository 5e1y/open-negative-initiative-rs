"""spektrafilm's print of Portra 800 dye amounts on each paper, the fixture ONI's paper is checked against:
paper_fixture.py <paper>... > crates/oni-pipeline/tests/paper/spektrafilm.json, in a venv with spektrafilm installed."""
import dataclasses, json, sys
import numpy as np
from spektrafilm.runtime.params_builder import digest_params, init_params
from spektrafilm.runtime.pipeline import SimulationPipeline
from spektrafilm.model.illuminants import standard_illuminant
from spektrafilm.model.color_filters import custom_dichroic_filters
from spektrafilm.model.develop import develop_simple

def pipe(paper, compressed=True):
    p = init_params(film_profile="kodak_portra_800", print_profile=paper)
    p.debug.lut_mode = True
    p.io.input_color_space = "ITU-R BT.2020"; p.io.input_cctf_decoding = False
    p.io.output_color_space = "ITU-R BT.2020"; p.io.output_cctf_encoding = False
    if not compressed:
        p.io.output_gamut_compress = dataclasses.replace(p.io.output_gamut_compress, algorithm="off")
    return SimulationPipeline(digest_params(p))

rounded = lambda a: np.round(np.asarray(a, dtype=float), 9).tolist()
rng = np.random.default_rng(7)
amounts = np.concatenate([np.zeros((1, 3)), rng.uniform(0.0, 2.6, (14, 3)),
                          np.array([[0.5, 0.9, 1.2], [1.0, 1.0, 1.0], [2.5, 0.2, 0.1], [0.1, 2.4, 0.3], [0.2, 0.3, 2.6]])])
out = {"source": "spektrafilm 3bb2c2d, Kodak Portra 800 dye amounts printed on each paper in lut_mode (TH-KG3, the database "
                 "neutral filters, print exposure 1), docs/research/colour-diagnosis/paper_fixture.py",
       "amounts": rounded(amounts), "lamp": rounded(standard_illuminant("TH-KG3")), "dichroics": rounded(custom_dichroic_filters.filters),
       "papers": {}}
for paper in sys.argv[1:]:
    pp = pipe(paper)
    e, f = pp._params.enlarger, pp._params.film
    # The film's mid grey as spektrafilm normalises the print on it: its curves at the grey, their least density kept.
    raw = pp._filming_stage._rgb_to_film_raw(np.array([[[0.184] * 3]]))
    cmy_mid = develop_simple(np.log10(raw + 1e-10), f.data.log_exposure, f.data.density_curves, 1.0)[0, 0]
    tap = lambda collect: pp.process(amounts[None], inject="cmy_film", collect=collect)[0]
    out["papers"][paper] = {"filters": [e.c_filter_neutral, e.m_filter_neutral, e.y_filter_neutral], "cmy_mid": rounded(cmy_mid),
                            "log_e_print": rounded(tap("log_e_print")), "cmy_print": rounded(tap("cmy_print")),
                            "rgb_uncompressed": rounded(pipe(paper, compressed=False).process(amounts[None], inject="cmy_film")[0]),
                            "rgb": rounded(tap(None))}
json.dump(out, sys.stdout, separators=(",", ":"))
