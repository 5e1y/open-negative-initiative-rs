"""ONI's print rebuilt in steps toward the paper: grey balance on a whole ramp, the paper's tone, matrices."""
import numpy as np, lab, spk
from chains import *

STOPS = np.arange(-9.0, 7.01, 0.05)
RAMPS = 0.184 * 2.0 ** STOPS
RAMP3 = np.repeat(RAMPS[:, None], 3, 1)
c_ramp, c_anch = cmy(RAMP3), cmy(ANCH)

def equivalent(light, ramp_light):
    """Each channel's reading as the grey exposure, in stops, that reads the same: greys equal at every level."""
    d, dr = -np.log10(light), -np.log10(ramp_light)
    return np.stack([np.interp(d[..., k], dr[:, k], STOPS) for k in range(3)], -1)

oni_ramp, spk_ramp = read_oni(c_ramp), read_spk(c_ramp)
B_ramp = oni_tone(oni_ramp, read_oni(c_anch))
C_ramp = spk.run(cpipe, RAMP3)
YB = B_ramp @ lab.M2X[1]; YC = C_ramp @ lab.M2X[1]
tone_oni = lambda e: np.interp(e, STOPS, YB)
tone_paper = lambda e: np.interp(e, STOPS, YC)

def mat(m, e):
    return np.einsum("ij,...j->...i", m, e)
