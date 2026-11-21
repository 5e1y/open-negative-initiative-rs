"""A real scan of the virtual negative: a light table, a camera's measured sensitivities, the camera's
matrix fitted on the ColorChecker's reflectances as a raw converter's is, then linear Rec. 2020."""
import numpy as np, colour, lab
from chains import cpipe

SHAPE = colour.SpectralShape(380, 780, 5)
WL = SHAPE.wavelengths
film = cpipe.film.data
DYES = np.asarray(film.channel_density, dtype=float)
BASE = np.asarray(film.base_density, dtype=float)
VALID = np.isfinite(DYES).all(1) & np.isfinite(BASE)

def aligned(sd):
    return sd.copy().align(SHAPE, extrapolator_kwargs={"method": "Constant", "left": 0, "right": 0}).values

D50 = aligned(colour.SDS_ILLUMINANTS["D50"])
D65 = aligned(colour.SDS_ILLUMINANTS["D65"])
CMFS = colour.MSDS_CMFS["CIE 1931 2 Degree Standard Observer"].copy().align(SHAPE).values

def sensor(name):
    if name == "gauss":
        return np.stack([np.exp(-0.5 * ((WL - m) / 15) ** 2) for m in (610, 540, 455)], -1)
    m = colour.MSDS_CAMERA_SENSITIVITIES[name].copy().align(SHAPE, extrapolator_kwargs={"method": "Constant", "left": 0, "right": 0})
    return m.values

def camera_matrix(s):
    """Raw to XYZ fitted on the 24 reflectances under D65, the white kept: a DNG ColorMatrix's role."""
    cc = colour.SDS_COLOURCHECKERS["BabelColor Average"]
    R = np.stack([aligned(v) for v in cc.values()])
    raw = (R * D65) @ s
    xyz = (R * D65) @ CMFS / (D65 @ CMFS[:, 1])
    wraw = D65 @ s
    raw_n = raw / wraw
    M, *_ = np.linalg.lstsq(raw_n, xyz, rcond=None)
    wxyz = D65 @ CMFS / (D65 @ CMFS[:, 1])
    M = M * (wxyz / (np.ones(3) @ M))[None, :]
    return M.T, wraw

def scan(cmy, name="Nikon 5100 (NPL)", light=D50):
    s = sensor(name)
    M, wraw = camera_matrix(s)
    d = np.einsum("...k,lk->...l", cmy, np.nan_to_num(DYES)) + np.nan_to_num(BASE)
    t = np.where(VALID, 10.0 ** -d, 0.0)
    raw = (t * light) @ s / wraw
    xyz = np.einsum("ij,...j->...i", M, raw)
    return np.einsum("ij,...j->...i", np.linalg.inv(lab.M2X), xyz)
