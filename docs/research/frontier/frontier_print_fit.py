"""The Frontier SP-3000's table refitted as a paper of ONI's paper kernel (frontier-paper.md): the scanner's matrix and three
curves, each layer's density read through a pure dye on its own Rec. 2020 channel, written into the
"frontier" block of its paper's file, crates/oni-settings/stocks/fujifilm-frontier-sp-3000.json, its provenance kept.
Usage: python3 frontier_print_fit.py [rounds]."""

import json
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
SCANNER = REPO / 'crates/oni-pipeline/resources/scanner'
TERMS = 3   # normal integrals a layer's curve
ROUNDS = int(sys.argv[1]) if len(sys.argv) > 1 else 300
# Fit weights: the neutral axis (channel spread under 0.1), nodes every measurement reads (under 0.6), mostly read (under
# 1.0), the rest of the reach. Without the axis' own weight the fit trades the greys' tone for the colours.
WEIGHTS = (20.0, 1.0, 0.5, 0.1)

cube = np.fromfile(SCANNER / 'frontier.cube', dtype='<f4').reshape(27, 27, 27, 4)[..., :3]  # [b, g, r]
meta = json.loads((SCANNER / 'frontier.json').read_text())
lo, hi = meta['render']['densityRange']
reached_low, reached_high = np.array(meta['render']['reached'])
neutral = float(meta['neutral']['density'][1])
nodes = lo + (hi - lo) * np.arange(27) / 26
B, G, R = np.meshgrid(nodes, nodes, nodes, indexing='ij')
X = np.stack([R, G, B], -1).reshape(-1, 3)
CODE = cube.reshape(-1, 3).astype(float)
inside = np.all((X >= reached_low - 1e-9) & (X <= reached_high + 1e-9), 1)
spread = X.max(1) - X.min(1)

TO2020 = np.array([[0.627404, 0.329283, 0.043313], [0.069097, 0.919540, 0.011362], [0.016391, 0.088013, 0.895595]])
TO709 = np.linalg.inv(TO2020)
OK1 = np.array([[0.4122214708, 0.5363325363, 0.0514459929], [0.2119034982, 0.6806995451, 0.1073969566], [0.0883024619, 0.2817188376, 0.6299787005]])
OK2 = np.array([[0.2104542553, 0.7936177850, -0.0040720468], [1.9779984951, -2.4285922050, 0.4505937099], [0.0259040371, 0.7827717662, -0.8086757660]])


def linear(code):
    c = np.clip(code, 0, 1)
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def oklab(rec709):
    return np.cbrt(rec709 @ OK1.T) @ OK2.T


def shown(rec2020, clipped=True):
    """Oklab of a linear Rec. 2020 light as the display clips it, or as it is."""
    rec709 = rec2020 @ TO709.T
    return oklab(np.clip(rec709, 0, 1) if clipped else rec709)


TARGET = oklab(linear(CODE))


def erf(x):
    # Abramowitz and Stegun 7.1.26, within 1.5e-7.
    s, x = np.sign(x), np.abs(x)
    t = 1 / (1 + 0.3275911 * x)
    return s * (1 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * np.exp(-x * x))


def unpack(p):
    m = np.eye(3)
    m[0, 1], m[0, 2], m[1, 0], m[1, 2], m[2, 0], m[2, 1] = p[:6]
    for r in range(3):
        m[r, r] = 1 - (m[r].sum() - m[r, r])
    i = 6
    curves = p[i:i + 9 * TERMS].reshape(3, TERMS, 3)  # log amplitude, centre, log sigma
    return m, curves


def developed(e, curves):
    d = np.zeros_like(e)
    for c in range(3):
        for a, mu, ls in curves[c]:
            d[:, c] += np.exp(a) * 0.5 * (1 + erf((e[:, c] - mu) / (np.exp(ls) * np.sqrt(2))))
    return d


def printed(p, x):
    m, curves = unpack(p)
    return 10 ** -developed(-(x - neutral) @ m.T, curves)


def start():
    p = [0.0] * 6
    for _ in range(3):
        for k in range(TERMS):
            p += [np.log(3.6 / TERMS), -0.5 + (k + 0.5) / TERMS, np.log(0.35)]
    return np.array(p)


def levenberg_marquardt(p, x, target, w):
    def residual(q):
        return ((shown(printed(q, x), clipped=False) - target) * 100 * w[:, None]).ravel()

    lam, r = 1e-2, residual(p)
    cost = r @ r
    for _ in range(ROUNDS):
        jac = np.empty((r.size, p.size))
        for j in range(p.size):
            q = p.copy()
            h = 1e-5 * max(1.0, abs(p[j]))
            q[j] += h
            jac[:, j] = (residual(q) - r) / h
        a, g = jac.T @ jac, jac.T @ r
        while True:
            q = p + np.linalg.solve(a + lam * np.diag(np.diag(a) + 1e-9), -g)
            rq = residual(q)
            if rq @ rq < cost:
                p, r, cost, lam = q, rq, rq @ rq, max(lam / 3, 1e-7)
                break
            lam *= 4
            if lam > 1e8:
                return p
    return p


def figures(p, sel):
    e = np.linalg.norm(shown(printed(p, X[sel])) - TARGET[sel], axis=1) * 100
    r = lambda v: round(float(v), 3)
    return {'median': r(np.median(e)), 'p90': r(np.quantile(e, 0.9)), 'p99': r(np.quantile(e, 0.99)), 'max': r(e.max()), 'nodes': int(sel.sum())}


def main():
    sel = inside
    w = np.select([spread[sel] < 0.1, spread[sel] < 0.6, spread[sel] < 1.0], WEIGHTS[:3], WEIGHTS[3])
    p = levenberg_marquardt(start(), X[sel], TARGET[sel], w)
    p = np.round(p, 6)
    m, curves = unpack(p)
    sets = {'neutral': inside & (spread < 0.1), 'constrained': inside & (spread < 0.6), 'mostlyRead': inside & (spread < 1.0), 'reached': inside}
    out = {
        'about': 'The Frontier SP-3000 refitted from frontier.cube as a paper of ONI paper kernel: x the negative density above its base; '
                 'log exposure -(matrix (x - neutral)); each layer developed on a sum of normal integrals; each read through a pure dye on its own channel of linear Rec. 2020.',
        'neutral': neutral,
        'matrix': m.round(6).tolist(),
        'curves': [[[float(np.exp(a)), float(mu), float(np.exp(ls))] for a, mu, ls in c] for c in curves],
        'figures': {name: figures(p, s) for name, s in sets.items()},
        'fit': {'terms': TERMS, 'rounds': ROUNDS, 'weights': WEIGHTS, 'metric': 'Oklab distance x 100 of display-clipped code',
                'nodes': 'frontier.cube inside its reach; neutral: channel spread under 0.1; constrained: under 0.6, every node read by a measurement'},
    }
    paper_file = REPO / 'crates/oni-settings/stocks/fujifilm-frontier-sp-3000.json'
    paper = json.loads(paper_file.read_text())
    paper['frontier'] = {'provenance': paper['frontier']['provenance'], **out}
    paper_file.write_text(json.dumps(paper, indent=1, ensure_ascii=False) + '\n')
    print(json.dumps(out['figures'], indent=1))


if __name__ == '__main__':
    main()
