"""B, C and their hybrids: reading (colorimetric scan or the paper's printing densities), tone (ONI's windows
and 2.2 power, or the paper's curves), output (Rec. 2020 primaries, or the paper's dyes seen under D50)."""
import numpy as np, lab, spk
from base import names, P, B, C, PE, CHART, PH, RAMP, cpipe, table

ANCH = np.array([[0.0] * 3, [0.184] * 3])
spipe = spk.pipeline(scan=True)
SHARE = lab.share()

def cmy(scene):
    return spk.run(cpipe, scene, collect="cmy_film")

def read_oni(c):
    """The colorimetric scan of the negative: Rec. 2020 light."""
    return spk.run(spipe, c, inject="cmy_film")

def read_spk(c):
    """The paper's own reading: each layer's exposure through the negative, as light."""
    return 10 ** spk.run(cpipe, c, inject="cmy_film", collect="log_e_print")

def oni_tone(light, anchors, sep=None, decomp=None):
    gain, black, white = lab.reading(anchors[0], anchors[1], SHARE)
    d = lab.density(light, gain)
    t = np.maximum((d - black) / (white - black), 0.0)
    if decomp is not None:
        t = decompressed(d, t, black, white, *decomp)
    if sep is not None:
        t = np.einsum("ij,...j->...i", sep, t)
    return np.maximum(t, 0.0) ** lab.GAMMA

LUMA = np.array([0.2627, 0.6780, 0.0593])
def decompressed(film, e, low, high, dose, share):
    """stage47's chroma decompression with no roll reference: (dose, 1 - dose)."""
    span = high - low
    place = (film - low) / span
    mean = place.mean(-1, keepdims=True)
    neutral = low + mean * span
    toned = np.maximum((neutral - low) / span, 0.0)
    held = toned + dose * (e - toned) + share * (film - neutral)
    return held + ((e - held) @ LUMA)[..., None]

def paper(light, anchors):
    """Any reading laid on the paper: each channel's log light mapped affinely so its bare film and grey land
    where the paper's own reading puts them, then the paper's curves, dyes and D50 scan."""
    own = np.log10(read_spk(cmy(ANCH)))
    q, qa = np.log10(light), np.log10(anchors)
    k = (own[1] - own[0]) / (qa[1] - qa[0])
    return spk.run(cpipe, own[0] + (q - qa[0]) * k, inject="log_e_print")

def paper_direct(light, anchors):
    """The paper's curves, its dyes taken as pure Rec. 2020 primaries: 10^-density above the paper's white."""
    own = np.log10(read_spk(cmy(ANCH)))
    q, qa = np.log10(light), np.log10(anchors)
    k = (own[1] - own[0]) / (qa[1] - qa[0])
    d = spk.run(cpipe, own[0] + (q - qa[0]) * k, inject="log_e_print", collect="cmy_print")
    dw = spk.run(cpipe, np.full((1, 3), -10.0), inject="log_e_print", collect="cmy_print")[0]
    out = 10 ** -(d - dw)
    g = C[RAMP][4]
    gd = 10 ** -(spk.run(cpipe, own[1:2], inject="log_e_print", collect="cmy_print")[0] - dw)
    return out * (g / gd)

def summary(x, ref=None, label=""):
    ref = C if ref is None else ref
    d = lab.de(x, ref)
    print(f"{label:52s} dE00 to C: chart {d[CHART].mean():5.2f} (max {d[CHART].max():5.1f})  photo {d[PH].mean():5.2f}  greys {d[RAMP].mean():5.2f}")
    return d
