"""spektrafilm's enlarger, the fixture ONI's lamp and dichroic filters are checked against:
paper_fixture.py > crates/oni-pipeline/tests/paper/spektrafilm.json, in a venv with spektrafilm installed."""
import json, sys
import numpy as np
from spektrafilm.model.illuminants import standard_illuminant
from spektrafilm.model.color_filters import custom_dichroic_filters

rounded = lambda a: np.round(np.asarray(a, dtype=float), 9).tolist()
out = {"source": "spektrafilm 3bb2c2d: its TH-KG3 enlarger lamp and its dichroic filters on the 380 to 780 nm grid, "
                 "docs/research/colour-diagnosis/paper_fixture.py",
       "lamp": rounded(standard_illuminant("TH-KG3")), "dichroics": rounded(custom_dichroic_filters.filters)}
json.dump(out, sys.stdout, separators=(",", ":"))
sys.stdout.write("\n")
