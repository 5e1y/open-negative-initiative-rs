"""Both chains on one set of patches: the chart, the photo's patches, a grey ramp; Lab on D65."""
import os
import numpy as np, colour
import spk
from io32 import read

R2020 = colour.RGB_COLOURSPACES["ITU-R BT.2020"]
M2X = R2020.matrix_RGB_to_XYZ
D65 = colour.CCS_ILLUMINANTS["CIE 1931 2 Degree Standard Observer"]["D65"]

def chart():
    cc = colour.CCS_COLOURCHECKERS["ColorChecker24 - After November 2014"]
    names = list(cc.data.keys())
    xyz = colour.xyY_to_XYZ(np.array(list(cc.data.values())))
    xyz = colour.chromatic_adaptation(xyz, colour.xy_to_XYZ(cc.illuminant), colour.xy_to_XYZ(D65), method="Von Kries", transform="Bradford")
    rgb = colour.XYZ_to_RGB(xyz, R2020, illuminant=D65, chromatic_adaptation_transform=None)
    return names, rgb

PHOTO = {"sky": (1085, 96), "cap": (1157, 277), "cheek": (1228, 385), "forearm": (1301, 723), "tee": (1060, 723),
         "wing white": (1446, 361), "fuselage red": (420, 400), "asphalt sun": (1450, 900), "asphalt shade": (361, 1012),
         "grass": (1494, 506), "trees": (60, 230)}
F = os.environ["ONI_COLOUR_PHOTO"]  # film_compare's output folder for one photo

def box(img, xy, r=6):
    x, y = xy
    return np.median(img[y - r:y + r + 1, x - r:x + r + 1].reshape(-1, 3), axis=0)

def photo():
    s = read(F + "/scene-rest.f32")
    return list(PHOTO), np.array([box(s, xy) for xy in PHOTO.values()])

def patches():
    n1, c = chart()
    n2, p = photo()
    ramp = 0.184 * 2.0 ** np.arange(-4, 4.5, 1.0)
    n3 = [f"grey {e:+.0f}" for e in np.arange(-4, 4.5, 1.0)]
    return n1 + n2 + n3, np.concatenate([c, p, np.repeat(ramp[:, None], 3, 1)])

def lab(rgb2020):
    return colour.XYZ_to_Lab(np.einsum("ij,...j->...i", M2X, np.maximum(rgb2020, 0)), D65)

def lch(rgb2020):
    L, a, b = np.moveaxis(lab(rgb2020), -1, 0)
    return np.stack([L, np.hypot(a, b), np.degrees(np.arctan2(b, a)) % 360], -1)

def de(x, y):
    return colour.delta_E(lab(x), lab(y), method="CIE 2000")

# ONI's print of a scan at rest, as conversion::stage47 sets it for the virtual Portra 800 (colour_trace params).
PED, GUARD, GAMMA = 0.00031622776, 3.952847e-5, 2.2
ONI_GAIN = np.array([0.5570779, 1.0, 2.2931225])
ONI_BLACK = np.array([0.5436084, 0.54339576, 0.54277563])
ONI_WHITE = np.array([1.4544518, 1.6437365, 1.7300874])

def density(scan, gain):
    return -np.log10(np.maximum((scan + PED) * gain, GUARD))

def reading(bare, grey, share):
    """film::derived on another reading: gains making the bare film grey, black on it, white from the grey."""
    gain = np.array([bare[1] / bare[0], 1.0, bare[1] / bare[2]])
    b = density(bare, gain)
    w = b + (density(grey, gain) - b) / share
    return gain, b, w

def oni_print(scan, gain=ONI_GAIN, black=ONI_BLACK, white=ONI_WHITE, sep=None):
    d = density(scan, gain)
    t = np.maximum((d - black) / (white - black), 0.0)
    if sep is not None:
        t = np.einsum("ij,...j->...i", sep, t)
    return np.maximum(t, 0.0) ** GAMMA

_scan = spk.pipeline(scan=True)
def scan(scene):
    return spk.run(_scan, scene)

def share():
    """digital_share(0.184), read back from ONI's own reference."""
    bare, grey = scan(np.array([[0.0] * 3, [0.184] * 3]))
    d = density(grey, ONI_GAIN)
    return float(((d - ONI_BLACK) / (ONI_WHITE - ONI_BLACK))[1])
