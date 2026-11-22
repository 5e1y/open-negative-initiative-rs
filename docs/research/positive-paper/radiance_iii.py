"""Kodak Ektachrome Radiance III Paper (Kodak publication E-1766, February 2003) as a paper profile in spektrafilm's
layout, which ONI's paper reads unchanged: crates/oni-pipeline/resources/film/kodak_ektachrome_radiance_iii.json.

  python3 radiance_iii.py digitise E-1766.pdf   # the curves of page 5, read off their vector paths, into radiance_iii.csv
  python3 radiance_iii.py fit                   # radiance_iii.csv into the profile

The publication: https://125px.com/docs/paper/kodak/e1766.pdf. Its three figures are vector drawings, so each curve is
read at the points Kodak drew, not by eye. Reading the PDF takes pymupdf; the fit numpy alone.
"""

import csv
import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
CSV = HERE / 'radiance_iii.csv'
OUT = REPO / 'crates/oni-pipeline/resources/film/kodak_ektachrome_radiance_iii.json'

# Each figure's frame on page 5, in PDF points, and the values at its edges: (x0, x1, y0, y1) -> (u0, u1, v0, v1).
# Read off the frames and their ticks (characteristic: a log exposure and a density a 46.1 points; dyes: 61.45
# points a 100 nm, 73.76 a unit of density; sensitivity: 0.401 points a nm, 37.875 a log unit).
FIGURES = {
    'density': ((86.6, 271.1, 263.8, 79.3), (0.0, 4.0, 0.0, 4.0)),
    'dye': ((362.8, 547.1, 266.3, 81.9), (400.0, 700.0, 0.0, 2.5)),
    'sensitivity': ((224.7, 425.2, 496.0, 344.5), (250.0, 750.0, -2.0, 2.0)),
}
CURVE_WIDTH = 0.72  # the curves' stroke; frames, ticks and leaders are thinner


def digitise(pdf):
    import pymupdf

    page = pymupdf.open(pdf)[4]
    paths = []
    for drawing in page.get_drawings():
        if drawing.get('width') is None or abs(drawing['width'] - CURVE_WIDTH) > 1e-3:
            continue
        current = []
        for item in drawing['items']:
            if item[0] != 'l':
                continue
            a, b = item[1], item[2]
            if current and (abs(current[-1][0] - a.x) > 0.05 or abs(current[-1][1] - a.y) > 0.05):
                paths.append(current)
                current = []
            if not current:
                current.append((a.x, a.y))
            current.append((b.x, b.y))
        if current:
            paths.append(current)
    rows = []
    for path in paths:
        x, y = np.array(path).T
        if len(path) < 20:
            continue  # a label's leader
        for figure, ((x0, x1, y0, y1), (u0, u1, v0, v1)) in FIGURES.items():
            if x.min() >= min(x0, x1) - 1 and x.max() <= max(x0, x1) + 1 and y.min() >= min(y0, y1) - 1 and y.max() <= max(y0, y1) + 1:
                u = u0 + (x - x0) / (x1 - x0) * (u1 - u0)
                v = v0 + (y - y0) / (y1 - y0) * (v1 - v0)
                rows.append((figure, u, v))
    named = name_curves(rows)
    with CSV.open('w', newline='') as f:
        w = csv.writer(f)
        w.writerow(['figure', 'curve', 'x', 'y'])
        for figure, curve, u, v in named:
            for a, b in zip(u, v):
                w.writerow([figure, curve, f'{a:.4f}', f'{b:.4f}'])


def name_curves(rows):
    """Each curve named as the figure labels it: density R, G and B by their Dmax (R highest, G lowest, as labelled);
    dyes by where they peak; sensitivities by their band."""
    out = []
    density = sorted([r for r in rows if r[0] == 'density'], key=lambda r: r[2][0])
    for (figure, u, v), name in zip(density, ['G', 'B', 'R']):
        out.append((figure, name, u, v))
    for figure, u, v in rows:
        if figure == 'dye':
            peak = u[np.argmax(np.where(u > 430, v, -1))]
            out.append((figure, 'yellow' if peak < 500 else 'magenta' if peak < 600 else 'cyan', u, v))
        elif figure == 'sensitivity':
            mid = 0.5 * (u.min() + u.max())
            out.append((figure, 'blue' if mid < 450 else 'green' if mid < 560 else 'red', u, v))
    assert sorted(c for _, c, _, _ in out) == sorted(['R', 'G', 'B', 'cyan', 'magenta', 'yellow', 'red', 'green', 'blue']), out
    return out


def read_csv():
    curves = {}
    with CSV.open() as f:
        for row in csv.DictReader(f):
            curves.setdefault(row['curve'], []).append((float(row['x']), float(row['y'])))
    return {k: np.array(sorted(v)) for k, v in curves.items()}


# ISO 5-3 Status A, log10 spectral products normalised to a 5.000 peak (ANSI/ISO 5-3-1995, Table 3), each band's
# tabulated range and the slopes past its ends, per nm.
STATUS_A = {
    'blue': (420, [3.602, 4.819, 5.000, 4.912, 4.620, 4.040, 2.989, 1.566, 0.165], 0.380, -0.140),
    'green': (500, [1.650, 3.822, 4.782, 5.000, 4.906, 4.644, 4.221, 3.609, 2.766, 1.579], 0.220, -0.170),
    'red': (600, [2.568, 4.638, 5.000, 4.871, 4.604, 4.286, 3.900, 3.551, 3.165, 2.776, 2.383, 1.970, 1.551, 1.141, 0.741, 0.341], 0.270, -0.040),
}
GRID = np.arange(380.0, 781.0, 5.0)
DYE_GRID = GRID[(GRID >= 400) & (GRID <= 700)]


def status_a(band, w):
    first, values, below, above = STATUS_A[band]
    last = first + 10 * (len(values) - 1)
    inside = np.interp(w, first + 10 * np.arange(len(values)), values)
    log = np.where(w < first, values[0] + below * (w - first), np.where(w > last, values[-1] + above * (w - last), inside))
    return 10.0 ** log


WEIGHTS = np.stack([status_a(b, DYE_GRID) for b in ('red', 'green', 'blue')])  # cyan, magenta, yellow read through R, G, B


def reading(spectral):
    """Status A densities of spectral densities on DYE_GRID, the last axis wavelength."""
    t = 10.0 ** -spectral
    return -np.log10((t[..., None, :] * WEIGHTS).sum(-1) / WEIGHTS.sum(-1))


def amounts(integral, dyes, base, start):
    """The dye amounts whose print reads `integral` in Status A, by Newton's steps."""
    a = np.array(start, float)
    for _ in range(50):
        r = reading(a @ dyes + base) - integral
        if np.abs(r).max() < 1e-10:
            break
        j = np.stack([(reading((a + h) @ dyes + base) - reading(a @ dyes + base)) / 1e-6 for h in np.eye(3) * 1e-6], 1)
        a = a - np.linalg.solve(j, r)
    return a


def cdf(z):
    # Abramowitz and Stegun 7.1.26 for erf, within 1.5e-7.
    x = np.abs(z) / np.sqrt(2)
    t = 1 / (1 + 0.3275911 * x)
    erf = 1 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * np.exp(-x * x)
    return 0.5 * (1 + np.sign(z) * erf)


TERMS = 3      # falling normal integrals a layer
STAIN_AT = 50.0  # a term centred this far past the axis is constant on it: the layer's stain at full light


def model(p, e):
    a, c, s = p.reshape(3, -1)
    return (a[:, None] * cdf((e[None, :] - c[:, None]) / s[:, None])).sum(0)


def fit_layer(e, d):
    """Three falling normal integrals and the stain, by Levenberg-Marquardt on the samples."""
    stain = max(d[-1], 0.0)
    falling = d - stain
    top = falling[0]
    p = np.array([top / 3] * TERMS + list(np.interp(top * np.array([0.8, 0.5, 0.2]), falling[::-1], e[::-1])) + [-0.3] * TERMS)
    lam = 1e-3
    for _ in range(400):
        r = model(p, e) + stain - d
        j = np.stack([(model(p + h, e) - model(p, e)) / 1e-7 for h in np.eye(len(p)) * 1e-7], 1)
        while True:
            step = np.linalg.solve(j.T @ j + lam * np.diag(np.diag(j.T @ j)), -j.T @ r)
            q = p + step
            if np.all(q[2 * TERMS:] < -1e-3) and np.sum((model(q, e) + stain - d) ** 2) < np.sum(r ** 2):
                p, lam = q, lam / 3
                break
            lam *= 4
            if lam > 1e12:
                break
        if lam > 1e12:
            break
    a, c, s = p.reshape(3, -1)
    return list(a) + [stain], list(c) + [STAIN_AT], list(s) + [-1.0], np.abs(model(p, e) + stain - d).max()


def resample_sensitivity(u, v):
    """A layer's log sensitivity on the grid: Kodak's curve inside its range, past each end the slope of its last 10 nm,
    never rising away from the band."""
    out = np.interp(GRID, u, v)
    near = lambda lo, hi: (np.interp(hi, u, v) - np.interp(lo, u, v)) / (hi - lo)
    rise, fall = max(near(u[0], u[0] + 10), 0.0), min(near(u[-1] - 10, u[-1]), 0.0)
    out = np.where(GRID < u[0], v[0] + rise * (GRID - u[0]), out)
    return np.where(GRID > u[-1], v[-1] + fall * (GRID - u[-1]), out)


def fit():
    c = read_csv()
    raw = np.stack([np.interp(DYE_GRID, *c[name].T) for name in ('cyan', 'magenta', 'yellow')])
    # Each dye scaled to read a Status A density of one through its own channel alone.
    scale = np.array([1.0, 1.0, 1.0])
    for k in range(3):
        lo, hi = 0.1, 3.0
        for _ in range(80):
            mid = 0.5 * (lo + hi)
            lo, hi = (mid, hi) if reading(mid * raw[k])[k] < 1.0 else (lo, mid)
        scale[k] = 0.5 * (lo + hi)
    dyes = raw * scale[:, None]
    # The integral densities on a shared log exposure, cyan, magenta and yellow layers read in R, G and B.
    e = np.linspace(0.0, 3.8, 191)
    integral = np.stack([np.interp(e, *c[name].T) for name in ('R', 'G', 'B')], 1)
    # The base: flat, as Kodak publishes none, at the density that leaves the full light's least layer with no dye.
    floor = integral[-1]
    lo, hi = 0.0, floor.min()
    for _ in range(60):
        b = 0.5 * (lo + hi)
        lo, hi = (b, hi) if amounts(floor, dyes, b, [0.01] * 3).min() > 0 else (lo, b)
    base = 0.5 * (lo + hi)
    analytic = np.zeros_like(integral)
    start = [2.0] * 3
    for i, d in enumerate(integral):
        analytic[i] = start = amounts(d, dyes, base, start)
    # spektrafilm's layout reads a unit of exposure (log 0) where a layer develops a density of 1 over its least: the
    # axis moved so the neutral scale crosses its floor plus 1 there.
    neutral = integral.mean(1)
    shift = float(np.interp(-(neutral.min() + 1.0), -neutral, e))
    e = e - shift
    layers = [fit_layer(e, analytic[:, k]) for k in range(3)]
    for name, (_, _, _, worst) in zip(('cyan', 'magenta', 'yellow'), layers):
        print(f'{name}: the model within {worst:.4f} of the analytic density')
    sensitivity = np.stack([resample_sensitivity(*c[name].T) for name in ('red', 'green', 'blue')], 1)
    axis = np.linspace(-3.0, 3.0, 256)
    known = (GRID >= 400) & (GRID <= 700)
    channel = [[float(v) for v in dyes[:, list(DYE_GRID).index(w)]] if k else [None] * 3 for w, k in zip(GRID, known)]
    profile = {
        'metadata': {
            'source': 'Kodak Ektachrome Radiance III Paper, Kodak publication E-1766 (February 2003), page 5: characteristic, '
            'spectral-dye-density and spectral-sensitivity curves, read off the vector drawings by docs/research/positive-paper/radiance_iii.py.',
            'datasource': 'Original data are property of Eastman Kodak Company.',
            'derivation': 'Dyes scaled to a Status A density of 1 in their own channel (ISO 5-3 Status A); integral Status A densities '
            'turned into each layer\'s dye amount over a flat base; the log exposure moved so the neutral scale reads 1 over its floor at 0; '
            'each layer fitted by three falling normal integrals and its stain; '
            'log sensitivity resampled on 380 to 780 nm, extended past each end at its last slope.',
        },
        'info': {'stock': 'kodak_ektachrome_radiance_iii', 'name': 'Kodak Ektachrome Radiance III', 'type': 'positive', 'support': 'paper',
                 'stage': 'printing', 'use': 'still', 'densitometer': 'status_A', 'viewing_illuminant': 'D50'},
        'data': {
            'wavelengths': [float(w) for w in GRID],
            'log_sensitivity': [[float(v) for v in row] for row in sensitivity],
            'channel_density': channel,
            'base_density': [float(base) if k else None for k in known],
            'log_exposure': [float(v) for v in axis],
            'density_curves_model': {
                'model_type': 'cdfs',
                'centers': [layer[1] for layer in layers],
                'amplitudes': [layer[0] for layer in layers],
                'sigmas': [layer[2] for layer in layers],
            },
        },
    }
    OUT.write_text(json.dumps(profile, indent=1) + '\n')
    print(f'base {base:.4f}, dye scales {scale.round(4)}, Kodak\'s log exposure {shift:.4f} at 0, written to {OUT.relative_to(REPO)}')


if __name__ == '__main__':
    {'digitise': lambda: digitise(sys.argv[2]), 'fit': fit}[sys.argv[1]]()
