# stocks-from-family.py <stocks dir> <stocks-from-family.json> [--write] [--refit]: the colour negatives fitted on no reference.
# Each takes the mean couplers of its family's films fitted on one (colour-negative: photo, cine-negative: cine; three
# decimals), read from their stock files, and its own sheet's curves with their greys made neutral: each channel's window
# share along the greys becomes the three channels' mean ū, then g(ū; m, c), one contrast pass for the three, both ends held.
# (m, c) are the table's; --refit solves them again so the positive's slope and lightness at the mean grey reach its sisters'.
# Every file is checked to fold no curve; with --write, its `film.couplers` and `film.curves` take the new values.
import sys, json, os
import numpy as np

OFF = ~np.eye(3, dtype=bool)


def g(u, m, c):
    inside = np.clip(u, 0, 1)
    val = inside + m * 16 * inside**2 * (1 - inside)**2 + c * inside * (1 - inside) * (2 * inside - 1)
    return np.where(u > 1, 1 + (1 - c) * (u - 1), val)


def monotone(m, c):
    return bool(np.all(np.diff(g(np.linspace(0, 1, 401), m, c)) > 0))


def neutral_curves(le, sheet, e_sat, m, c):
    """The sheet above its D-min, its greys neutral and passed through g; the saturation's density kept per channel."""
    c0 = sheet - sheet.min(0)
    w = np.array([np.interp(e_sat, le, c0[:, k]) for k in range(3)])
    mean = (c0 / w).mean(1)
    bent = g(mean, m, c)
    return np.stack([np.maximum.accumulate(w[k] * bent) + 1e-6 * np.arange(len(le)) for k in range(3)], 1)


def folds(le, curves, k):
    """bake::folds: each layer's exposure before the couplers, log H − Σ D·K, must rise strictly."""
    return any(np.any(np.diff(le - curves @ k[:, c]) <= 0) for c in range(3))


def grey_slope_and_mid(le, curves, e_sat, stops):
    """The positive's grey ramp under the two points: its Oklab L slope per stop and its L at the mean grey."""
    import colour
    h = stops * np.log10(2)
    share = np.interp(h, le, curves[:, 1]) / np.interp(e_sat, le, curves[:, 1])
    lab = colour.XYZ_to_Oklab(colour.sRGB_to_XYZ(np.repeat(np.clip(share, 0, 1)[:, None], 3, 1)))
    i = int(np.argmin(np.abs(stops)))
    return (lab[i + 1, 0] - lab[i - 1, 0]) / (stops[i + 1] - stops[i - 1]), lab[i, 0]


def main():
    stocks, table = sys.argv[1], json.load(open(sys.argv[2]))
    write, refit = '--write' in sys.argv, '--refit' in sys.argv
    read = lambda key: json.load(open(os.path.join(stocks, key + '.json')))
    family = lambda stock: 'cine' if stock['type'] == 'cine-negative' else 'photo'
    fitted = {key: read(key) for key in table['with_reference']}
    mean_k = {}
    for fam in ('photo', 'cine'):
        terms = [np.array(s['film']['couplers'])[OFF] for s in fitted.values() if family(s) == fam]
        k = np.zeros((3, 3))
        k[OFF] = np.round(np.mean(terms, 0), table['k_decimals'])
        mean_k[fam] = k
    lo, hi, step = table['ramp_stops']
    stops = np.arange(lo, hi + step / 2, step)
    changed, bad = 0, []
    for key, entry in table['without_reference'].items():
        stock = read(key)
        data = stock['film']['profile']['data']
        le = np.array(data['log_exposure'], float)
        sheet = np.array(data['sheet_density_curves'], float)
        k = mean_k[family(stock)]
        m, c = entry['contrast_m_c']
        if refit:
            from scipy.optimize import least_squares
            level = table['sisters_level'][entry['sisters']]
            goal = np.array([level['slope'], level['mid_lightness']])
            def miss(v):
                if not monotone(*v):
                    return np.ones(2)
                return np.array(grey_slope_and_mid(le, neutral_curves(le, sheet, table['e_sat'], *v), table['e_sat'], stops)) / goal - 1
            starts = ([m, c], [0.01, 0.05], [0.05, 0.2], [0.08, -0.1], [0.03, 0.3])
            fit = min((least_squares(miss, x0, bounds=([-0.3, -0.8], [0.3, 0.8]), diff_step=0.02, x_scale=[0.05, 0.1]) for x0 in starts), key=lambda q: q.cost)
            m, c = fit.x
        curves = neutral_curves(le, sheet, table['e_sat'], m, c)
        if folds(le, curves, k):
            bad.append(key)
        new_sheet = np.round(curves + sheet.min(0), 6).tolist()
        new_k = np.round(k, 4).tolist()
        film = stock['film']
        same = film['couplers'] == new_k and film.get('curves') == new_sheet
        changed += not same
        print('%-30s %-5s m %+.4f c %+.4f %s' % (key, family(stock), m, c, 'unchanged' if same else 'differs from the file'))
        if write:
            film['couplers'] = new_k
            film['curves'] = new_sheet
            open(os.path.join(stocks, key + '.json'), 'w').write(json.dumps(stock, indent=1, ensure_ascii=False) + '\n')
    print('%d files, %d %s, folds: %s' % (len(table['without_reference']), changed, 'written' if write else 'differ', bad or 'none'))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
