"""Shared set-up: ONI_COLOUR_PHOTO is film_compare's folder for the photo, ONI_COLOUR_OUT holds colour_trace's
chart print (chart-oni/); every script runs in a venv with spektrafilm installed."""
import os
import numpy as np, lab, spk
from io32 import read
S = os.environ["ONI_COLOUR_OUT"]
names, P = lab.patches()
grey = np.array([[0.184] * 3])
B = read(S + "/chart-oni/printed.f32")[0]
Lb = lab.lab(lab.oni_print(lab.scan(grey)))[0, 0]
lo, hi = 0.02, 5.0
for _ in range(40):
    m = np.sqrt(lo * hi)
    L = lab.lab(spk.run(spk.pipeline(print_exposure=m), grey))[0, 0]
    lo, hi = (m, hi) if L > Lb else (lo, m)
PE = np.sqrt(lo * hi)
cpipe = spk.pipeline(print_exposure=PE)
C = spk.run(cpipe, P)
C1 = spk.run(spk.pipeline(), P)
CHART = slice(0, 24); PH = slice(24, 35); RAMP = slice(35, 44)
def table(cols, labels):
    print(f"{'patch':16s}" + "".join(f"{l:>20s}" for l in labels))
    for i, n in enumerate(names):
        print(f"{n:16s}" + "".join("   %5.1f %5.1f %5.0f" % tuple(lab.lch(c[i])) for c in cols))
