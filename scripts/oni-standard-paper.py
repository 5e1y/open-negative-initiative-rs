#!/usr/bin/env python3
"""ONI Standard, the default paper: a neutral paper whose characteristic curve is the mean shape of the six data-sheet
colour papers ONI ships (Royal Digital, Endura Premier, Endura Metallic, Ektacolor Edge, Photo Book, Crystal Archive
DPII), each read over its useful range (D-min + 0.04 to 0.9 Dmax on its grey-balanced neutral) as Prepare's window
lays the frame on it; a logistic in density fits their mean within 0.12 L* rms.

Over the window share s, 0 at Prepare's black and 1 at its white, D(s) = 2.484 / (1 + exp(5.587 (s - 0.354))). The
file lays it on log exposure x = 1 - s, its useful range [0, 1], the same curve on the three layers, pure dyes and no
D-min of its own: no colour cast. Run: scripts/oni-standard-paper.py, which writes
crates/oni-settings/stocks/oni-standard.json.
"""

import json
import math
from pathlib import Path

DENSITY, SLOPE, MIDDLE = 2.484, 5.587, 0.354
LOW, HIGH, NODES = -1.5, 2.5, 256


def density(x):
    return DENSITY / (1.0 + math.exp(SLOPE * ((1.0 - x) - MIDDLE)))


def main():
    step = (HIGH - LOW) / (NODES - 1)
    log_exposure = [round(LOW + step * i, 6) for i in range(NODES)]
    curves = [[round(density(x), 6)] * 3 for x in log_exposure]
    paper = {
        "oni": "paper",
        "format": 1,
        "key": "oni-standard",
        "name": "ONI Standard",
        "brand": "ONI",
        "profile": {
            "info": {"stock": "oni_standard", "name": "ONI Standard", "manufacturer": "ONI", "type": "paper", "densitometer": "status_A"},
            "provenance": {
                "model": "D(s) = 2.484 / (1 + exp(5.587 (s - 0.354))) over the window share s, 0 at Prepare's black, 1 at its white",
                "measured": [
                    "the mean of Royal Digital, Endura Premier, Endura Metallic, Ektacolor Edge, Photo Book and Crystal Archive DPII, "
                    "each over its useful range (D-min + 0.04 to 0.9 Dmax of its grey-balanced neutral), as light between its ends",
                ],
                "inferred": [
                    "a logistic in density through that mean, within 0.12 L* rms of it",
                    "the same curve on the three layers, pure dyes and no D-min of its own: a neutral paper",
                ],
                "script": "scripts/oni-standard-paper.py",
            },
            "data": {
                "log_exposure": log_exposure,
                "density_curves": curves,
                "crosstalk": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                "useful_range": [0.0, 1.0],
            },
        },
    }
    path = Path(__file__).resolve().parent.parent / "crates/oni-settings/stocks/oni-standard.json"
    path.write_text(json.dumps(paper, indent=1, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
