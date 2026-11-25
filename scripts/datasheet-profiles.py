#!/usr/bin/env python3
"""Colour negative film profiles from the makers' data sheets, in the shape ONI's bake reads.

Reads the emulsion dataset's export (filmstocks.json: curves traced from each official data sheet) and writes one
profile per stock, each with its sheet's reference, URL and SHA-256 and a list of what is measured and what is
inferred, into the "profile" of the stock's or the paper's file in crates/oni-settings/stocks/ (README.md there),
every other field of the file kept; a profile no file holds yet gets a file of its own, its name and brand to check.
A film whose profile moved has its reference derived again by the pipeline (examples/stock_references.rs).
Run: scripts/datasheet-profiles.py <emulsion-dataset folder>. Needs numpy and cargo.

What each part of a profile is:
- log_sensitivity: the sheet's three layers on 380-780 nm, linear in log between its points; past the first and
  last point each layer falls on at the slope of its last 15 nm, at least 0.02 log per nm (the sheet stops where
  the curve leaves its chart). Their levels are left as printed: the bake scales each layer so a grey under the
  stock's light exposes it at log 0, which is what the characteristic curves' common log H axis states.
- sheet_density_curves: the sheet's Status M curves less each channel's least density, on log H less the grey's;
  the grey sits 1.03 log above the ISO 5800 speed point of the curves themselves, the place Kodak's one printed
  "Log H Ref" (Gold 200, E-7022) holds over that sheet's speed point; a push sits n x 0.30 log under its stock's.
- density_curves: the dyes' amounts the bake develops, the sheet's curves unmixed: a Status M curve is a neutral's
  integral density, each dye's absorption in the other bands in it, so at each node the amounts are solved whose
  spectral density over the base Status M reads as the sheet prints, a dip the traces leave held at the point reached.
- channel_density: the dyes. A Vision3 sheet prints its cyan, magenta and yellow: their shapes are kept and their
  amounts fitted to the printed mid-scale neutral. Every other sheet prints only the mid-scale neutral and D-min:
  the Vision3 sheets' mean dyes are shifted and widened per dye (within 40 nm and 0.8-1.25, each held at its
  sheet's last value for 40 nm past it) to fit that neutral
  less D-min. Each dye is then scaled to a Status M density of 1 in its own channel, the amounts' unit.
- base_density: the sheet's D-min, printed.

A paper reading films holds its sheet's spectral sensitivities ("sensitivity" in its file), on the same grid and by the
same rule past its printed points; each still colour negative names its paper ("read_by": its maker's sheet's, else
the owner's choice, said in the profile's provenance) and states couplers of zero, and its filtration, each layer's
exposure offset that lays a mid grey on one share of the three windows through that paper's gaze under a 3200 K lamp,
is derived with its reference by the pipeline.

Status M and Status A are ISO 5-3's spectral products (ANSI/ISO 5-3-1995, tables 3 and 4), on the dyes' known
wavelengths. A colour paper's profile is its curves alone: its sheet's Status A characteristic curves as printed, each
on its own D-min, which ONI reads through pure dyes, each on its own channel; only a paper whose sheet prints them is
taken, and a display material viewed by transmission is written as a paper. Each layer's level is as printed: Kodak's
absolute, Fujifilm's on one relative axis for all three; the curves' log H origin is set where green prints an 18 %
grey over its D-min, the enlarger's time absorbing it. A dye's or a layer's points hidden on its sheet are filled by
its end slope; a reversal film's curve hidden under another's stroke takes that one's points.
"""

import json
import math
import subprocess
import sys
from pathlib import Path

import numpy as np

WAVELENGTHS = np.arange(380.0, 781.0, 5.0)
CURVE_NODES = 256
ROOT = Path(__file__).resolve().parent.parent
STOCKS_DIR = ROOT / "crates/oni-settings/stocks"
# The grey over the ISO 5800 speed point is measured on Gold 200's sheet, the only one printing its Log H Ref.
REFERENCE_SHEET = "kodak_gold_200"
LOG2 = math.log10(2.0)

# The stocks taken: the dataset's id, the file ONI names, and a push's stops with the chart it reads.
STOCKS = [
    ("fujifilm_160nc", None), ("fujifilm_160nl", None), ("fujifilm_160npc", None), ("fujifilm_160ns", None),
    ("fujifilm_200", None), ("fujifilm_400", None), ("fujifilm_c200", None), ("fujifilm_fujicolor_100", None),
    ("fujifilm_natura_1600", None), ("fujifilm_new_pro_400", None), ("fujifilm_nexia_400", None),
    ("fujifilm_nexia_a200", None), ("fujifilm_nexia_zoom_master_800", None), ("fujifilm_pro_160c", None),
    ("fujifilm_pro_160nc", None), ("fujifilm_pro_160nh", None), ("fujifilm_pro_160ns", None), ("fujifilm_pro_160s", None),
    ("fujifilm_pro_400h", None), ("fujifilm_pro_800", None), ("fujifilm_pro_800z", None), ("fujifilm_reala_ace", None),
    ("fujifilm_superia_100", None), ("fujifilm_superia_1600", None), ("fujifilm_superia_200", None),
    ("fujifilm_superia_premium_400", None), ("fujifilm_superia_reala", None), ("fujifilm_superia_venus_400", None),
    ("fujifilm_superia_venus_800", None), ("fujifilm_superia_x_tra_400", None), ("fujifilm_superia_x_tra_800", None),
    ("kodak_ektar_100", None), ("kodak_gold_200", None), ("kodak_portra_160", None), ("kodak_portra_160nc", None),
    ("kodak_portra_160vc", None), ("kodak_portra_400", None), ("kodak_portra_400nc", None), ("kodak_portra_400vc", None),
    ("kodak_portra_800", None), ("kodak_portra_800", 1), ("kodak_portra_800", 2), ("kodak_ultra_max_400", None),
    ("kodak_verita_200d_5206", None), ("kodak_vision3_200t_5213", None), ("kodak_vision3_250d_5207", None),
    ("kodak_vision3_500t_5219", None), ("kodak_vision3_50d_5203", None),
]

VISION3 = ["kodak_vision3_50d_5203", "kodak_vision3_200t_5213", "kodak_vision3_250d_5207", "kodak_vision3_500t_5219"]


def file_name(stock_id, push):
    base = stock_id.removesuffix("_5206").removesuffix("_5203").removesuffix("_5207").removesuffix("_5213").removesuffix("_5219")
    return base + (f"_push{push}" if push else "")


def series(chart, channel):
    xs, ys = [], []
    for x, y in zip(chart["x_values"], chart["channels"][channel]):
        if y is not None:
            xs.append(float(x))
            ys.append(float(y))
    return np.array(xs), np.array(ys)


def chart(stock, kind, chart_id=None):
    for c in stock["charts"]:
        if c["kind"] == kind and (chart_id is None or c["chart_id"] == chart_id):
            return c
    return None


def sensitivity(stock):
    """The three layers on the grid, red first as the bake reads them; each fills its sheet's gaps."""
    c = chart(stock, "spectral_sensitivity")
    out = []
    for channel in ("red_sensitive", "green_sensitive", "blue_sensitive"):
        x, y = series(c, channel)
        inside = np.interp(WAVELENGTHS, x, y)
        lo, hi = x[0], x[-1]

        def slope(at_end):
            near = (x >= hi - 15.0) if at_end else (x <= lo + 15.0)
            xs, ys = x[near], y[near]
            s = np.polyfit(xs, ys, 1)[0] if len(xs) >= 2 else 0.0
            return min(s, -0.02) if at_end else max(s, 0.02)

        tail_hi, tail_lo = slope(True), slope(False)
        filled = np.where(WAVELENGTHS > hi, y[-1] + tail_hi * (WAVELENGTHS - hi), inside)
        filled = np.where(WAVELENGTHS < lo, y[0] + tail_lo * (WAVELENGTHS - lo), filled)
        out.append(np.maximum(filled, y.max() - 8.0))
    return np.array(out).T, c


def speed_point(curves_chart):
    """ISO 5800's H_m: the geometric mean of the green's and the slower of red and blue's exposures at 0.15 over
    each channel's least density, in log H."""
    at = {}
    for k in "BGR":
        x, y = series(curves_chart, k)
        target = y.min() + 0.15
        i = int(np.argmax(y >= target))
        at[k] = float(np.interp(target, y[: i + 1], x[: i + 1]))
    return 0.5 * (at["G"] + max(at["B"], at["R"]))


def curves(curves_chart, grey):
    """The curves above each channel's least density on 256 nodes of log H less the grey's, red first; a traced
    dip under a point already reached is held at it, as a density never falls with exposure."""
    lows, highs = [], []
    channels = {}
    for k in "RGB":
        x, y = series(curves_chart, k)
        channels[k] = (x, np.maximum.accumulate(y) - y.min())
        lows.append(x[0])
        highs.append(x[-1])
    le = np.linspace(max(lows), min(highs), CURVE_NODES)
    d = np.array([np.interp(le, *channels[k]) for k in "RGB"]).T
    return le - grey, d


def spectrum(c, channel, grid):
    x, y = series(c, channel)
    return np.where((grid >= x[0]) & (grid <= x[-1]), np.interp(grid, x, y), np.nan)


def vision3_basis():
    """The Vision3 sheets' cyan, magenta and yellow, each at its peak 1, their mean on the grid where all print."""
    sets = []
    for sid in VISION3:
        c = chart(STOCK[sid], "spectral_dye_density")
        dyes = np.array([spectrum(c, ch, WAVELENGTHS) for ch in ("cyan", "magenta", "yellow")]).T
        sets.append(dyes / np.nanmax(dyes, axis=0))
    return np.mean(sets, axis=0)


# ISO 5-3's Status M and Status A spectral products (ANSI/ISO 5-3-1995, tables 3 and 4), red, green and blue: the
# first wavelength, the log10 products every 10 nm normalised to a 5.000 peak, and the log slopes past either end.
STATUS_M = [
    (620.0, [2.109, 4.479, 5.000, 4.899, 4.578, 4.252, 3.875, 3.491, 3.099, 2.687, 2.269, 1.859, 1.449, 1.054, 0.654, 0.254], 0.260, -0.040),
    (470.0, [1.152, 2.207, 3.156, 3.804, 4.272, 4.626, 4.872, 5.000, 4.995, 4.818, 4.458, 3.915, 3.172, 2.239, 1.070], 0.106, -0.120),
    (410.0, [2.103, 4.111, 4.632, 4.871, 5.000, 4.955, 4.743, 4.343, 3.743, 2.990, 1.852], 0.250, -0.220),
]
STATUS_A = [
    (600.0, [2.568, 4.638, 5.000, 4.871, 4.604, 4.286, 3.900, 3.551, 3.165, 2.776, 2.383, 1.970, 1.551, 1.141, 0.741, 0.341], 0.270, -0.040),
    (500.0, [1.650, 3.822, 4.782, 5.000, 4.906, 4.644, 4.221, 3.609, 2.766, 1.579], 0.220, -0.170),
    (420.0, [3.602, 4.819, 5.000, 4.912, 4.620, 4.040, 2.989, 1.566, 0.165], 0.380, -0.140),
]


def products(status):
    """A status's three spectral products on the grid, linear in log between the table's points, peak 1."""
    out = []
    for first, logs, before, after in status:
        nodes = first + 10.0 * np.arange(len(logs))
        v = np.interp(WAVELENGTHS, nodes, logs)
        v = np.where(WAVELENGTHS < nodes[0], logs[0] + before * (WAVELENGTHS - nodes[0]), v)
        v = np.where(WAVELENGTHS > nodes[-1], logs[-1] + after * (WAVELENGTHS - nodes[-1]), v)
        out.append(10.0 ** (v - 5.0))
    return np.array(out)


def read(spectral, status):
    """The three densities `status` reads of `spectral` density, on the wavelengths it is known."""
    w = products(status)
    ok = ~np.isnan(spectral)
    return -np.log10((w[:, ok] * 10.0 ** -spectral[ok]).sum(axis=1) / w[:, ok].sum(axis=1))


def unit(dyes, status):
    """Each dye scaled so `status` reads a density of 1 in its own channel."""
    out = dyes.copy()
    for c in range(3):
        for _ in range(50):
            out[:, c] /= read(out[:, c], status)[c]
    return out


def unmixed(net, base, dyes, status):
    """The dyes' amounts over `base` that `status` reads as `net` above the base's own reading, node by node: a
    printed curve is a neutral's integral density, each dye's unwanted absorptions in it. Newton's method on the
    band integrals, exact where a linear crosstalk matrix is not; a dip it draws is held, as for the curves."""
    w = products(status)
    ok = ~np.isnan(base) & ~np.isnan(dyes).any(axis=1)
    w, base, dyes = w[:, ok], base[ok], dyes[ok]
    own = -np.log10((w * 10.0 ** -base).sum(axis=1) / w.sum(axis=1))
    out = []
    a = np.zeros(3)
    for target in np.asarray(net) + own:
        for _ in range(50):
            t = 10.0 ** -(base + dyes @ a)
            seen = w * t
            r = -np.log10(seen.sum(axis=1) / w.sum(axis=1)) - target
            if np.abs(r).max() < 1e-10:
                break
            jacobian = (seen @ dyes) / seen.sum(axis=1)[:, None]
            a = a - np.linalg.solve(jacobian, r)
        else:
            raise SystemExit(f"the unmix does not converge on {target}")
        out.append(a.copy())
    # An amount never falls with exposure: a dip the traces' small disagreements leave is held at the point reached.
    return np.maximum.accumulate(np.array(out), axis=0)


def deformed(basis, shift, stretch):
    """A dye moved by `shift` nm and widened by `stretch` about its peak, NaN where the basis has no point."""
    ok = ~np.isnan(basis)
    peak = WAVELENGTHS[np.nanargmax(basis)]
    at = peak + (WAVELENGTHS - peak - shift) / stretch
    x, y = WAVELENGTHS[ok], basis[ok]
    return np.where((at >= x[0] - 40.0) & (at <= x[-1] + 40.0), np.interp(at, x, y), np.nan)


def amounts(dyes, target, band):
    """The dyes' amounts fitting `target` on `band`; none where a dye leaves the band unknown."""
    if np.isnan(dyes[band]).any():
        return np.zeros(3), math.inf
    ok = band
    a, *_ = np.linalg.lstsq(dyes[ok], target[ok], rcond=None)
    a = np.maximum(a, 0.0)
    residual = float(np.sqrt(np.mean((dyes[ok] @ a - target[ok]) ** 2)))
    return a, residual


def fit_dyes(basis, target, deform):
    """The three dyes fitted to `target`: their amounts, and with `deform` each dye's shift and width by turns."""
    params = [(0.0, 1.0)] * 3
    band = ~np.isnan(target) & ~np.isnan(basis).any(axis=1)
    shapes = lambda: np.array([deformed(basis[:, i], *params[i]) for i in range(3)]).T
    if deform:
        shifts = np.arange(-40.0, 40.1, 2.5)
        stretches = np.arange(0.8, 1.251, 0.05)
        for _ in range(6):
            for i in range(3):
                best = None
                for s in shifts:
                    for w in stretches:
                        trial = list(params)
                        trial[i] = (s, w)
                        dyes = np.array([deformed(basis[:, j], *trial[j]) for j in range(3)]).T
                        _, r = amounts(dyes, target, band)
                        cost = r * (1.0 + 0.01 * ((s / 40.0) ** 2 + (math.log(w) / math.log(1.25)) ** 2))
                        if best is None or cost < best[0]:
                            best = (cost, (s, w))
                params[i] = best[1]
    dyes = shapes()
    a, residual = amounts(dyes, target, band)
    return dyes, a, residual, params


def nulls(v):
    return [None if (isinstance(x, float) and math.isnan(x)) else round(float(x), 6) for x in v]


def convert(stock_id, push, reference_offset, basis):
    stock = STOCK[stock_id]
    measured, inferred = [], []
    log_sensitivity, sens_chart = sensitivity(stock)
    measured.append(f"spectral sensitivity, three layers, as printed ({sens_chart['confidence']} confidence)")
    inferred.append("each layer's sensitivity past the sheet's first and last point, falling at its end slope")
    if "fourth_layer" in sens_chart["channels"]:
        inferred.append("the sheet's fourth (cyan-sensitive) layer left out: its inhibition is not modelled")
    inferred.append("each layer's level: set by the bake so a grey under the stock's light exposes it at log 0")

    normal = chart(stock, "characteristic_curves", "characteristic_curves")
    curves_chart = chart(stock, "characteristic_curves", f"characteristic_curves_push{push}") if push else normal
    grey = speed_point(normal) + reference_offset - (push or 0) * LOG2
    le, density = curves(curves_chart, grey)
    measured.append(f"characteristic curves, Status M, as printed ({curves_chart['confidence']} confidence), each less its D-min")
    inferred.append(f"the grey's exposure: ISO 5800 speed point + {reference_offset:.2f} log (Kodak Gold 200's Log H Ref)"
                    + (f", less {push} stop{'s' if push > 1 else ''} for the push" if push else ""))
    inferred.append("densities held at the sheet's last point past the curves' printed range")

    dye_chart = chart(stock, "spectral_dye_density")
    base = spectrum(dye_chart, "minimum_density", WAVELENGTHS)
    mid = spectrum(dye_chart, "midscale_neutral", WAVELENGTHS)
    net = "subtracted" in (dye_chart["y"]["unit"] + dye_chart["condition"]).lower()
    target = mid if net else mid - base
    own = all(ch in dye_chart["channels"] for ch in ("cyan", "magenta", "yellow"))
    if own:
        printed = np.array([spectrum(dye_chart, ch, WAVELENGTHS) for ch in ("cyan", "magenta", "yellow")]).T
        dyes, a, residual, params = fit_dyes(printed / np.nanmax(printed, axis=0), target, deform=False)
        measured.append("cyan, magenta and yellow dye shapes, D-min and mid-scale neutral, as printed")
        inferred.append(f"the dyes' amounts, fitted to the mid-scale neutral (rms {residual:.4f} D)")
    else:
        dyes, a, residual, params = fit_dyes(basis, target, deform=True)
        measured.append("D-min and mid-scale neutral spectral densities, as printed")
        moves = ", ".join(f"{n} {s:+.1f} nm x{w:.2f}" for n, (s, w) in zip("CMY", params))
        inferred.append(f"the three dyes: the Vision3 sheets' mean dyes moved and widened ({moves}) to fit the "
                        f"mid-scale neutral less D-min (rms {residual:.4f} D)")
    band = ~np.isnan(base) & ~np.isnan(dyes).any(axis=1)
    dyes[~band] = np.nan
    base = np.where(band, base, np.nan)
    channel = unit(dyes, STATUS_M)
    inferred.append("each dye's unit: a Status M density of 1 in its own channel, by ISO 5-3's Status M products")
    amounts_ = unmixed(density, base, channel, STATUS_M)
    inferred.append("the dyes' amounts along log H: the curves unmixed, each node's amounts reading the sheet's Status M "
                    "densities over D-min through every dye's absorption in each band")

    name = file_name(stock_id, push)
    source = stock["source"]
    profile = {
        "info": {
            "stock": name,
            "name": stock["name"] + (f" (push {push})" if push else ""),
            "manufacturer": stock["manufacturer"],
            "type": "negative",
            "densitometer": "status_M",
            "reference_illuminant": "T" if name.endswith("t") else "D55",
        },
        "provenance": {
            "data_sheet": source.get("doc_id"),
            "url": source.get("url"),
            "sha256": source.get("sha256"),
            "retrieved": source.get("retrieved"),
            "dataset_id": stock_id,
            "charts": [c["chart_id"] for c in (sens_chart, curves_chart, dye_chart)],
            "measured": measured,
            "inferred": inferred,
        },
        "data": {
            "wavelengths": [float(w) for w in WAVELENGTHS],
            "log_sensitivity": [[round(float(v), 6) for v in row] for row in log_sensitivity],
            "channel_density": [nulls(row) for row in channel],
            "base_density": nulls(base),
            "log_exposure": [round(float(v), 6) for v in le],
            "density_curves": [[round(float(v), 6) for v in row] for row in amounts_],
            "sheet_density_curves": [[round(float(v), 6) for v in row] for row in density],
        },
    }
    place(profile)
    return name, inferred


# The colour papers taken: those whose sheet prints their characteristic curves and whose printed layers the
# enlarger's filtration balances (Fujiflex's and Fujitrans's, levelled for a laser, find no neutral rest).
PAPERS = [
    "kodak_endura_premier_paper",
    "kodak_endura_premier_metallic_paper",
    "kodak_ektacolor_edge_paper",
    "kodak_royal_digital_paper",
    "kodak_photo_book_paper",
    "fujifilm_crystal_archive_digital_paper_type_dpii",
    "fujifilm_crystal_archive_digital_pearl_paper",
]

def tailed(x, y, floor):
    """`y` on the grid, linear between its points, past them falling at its last 15 nm's slope in log, at least
    0.02 log per nm, never under `floor`."""
    inside = np.interp(WAVELENGTHS, x, y)
    near_hi, near_lo = x >= x[-1] - 15.0, x <= x[0] + 15.0
    hi = min(np.polyfit(x[near_hi], y[near_hi], 1)[0], -0.02) if near_hi.sum() >= 2 else -0.02
    lo = max(np.polyfit(x[near_lo], y[near_lo], 1)[0], 0.02) if near_lo.sum() >= 2 else 0.02
    out = np.where(WAVELENGTHS > x[-1], y[-1] + hi * (WAVELENGTHS - x[-1]), inside)
    out = np.where(WAVELENGTHS < x[0], y[0] + lo * (WAVELENGTHS - x[0]), out)
    return np.maximum(out, floor)


def convert_paper(stock_id):
    """A colour paper's profile: its sheet's Status A curves as printed, each on its own D-min, on log H less the
    grey's, and its dyes' Status A crosstalk: what each channel reads of each dye at a unit amount. ONI prints each layer
    through a pure dye on its own channel, so a paper's dye spectra and sensitivities are not kept."""
    stock = STOCK[stock_id]
    measured, inferred = [], []
    dye_chart = chart(stock, "spectral_dye_density")
    top = max(series(dye_chart, ch)[0][-1] for ch in ("cyan", "magenta", "yellow"))
    dyes = []
    for channel in ("cyan", "magenta", "yellow"):
        x, y = series(dye_chart, channel)
        logged = tailed(x, np.log10(np.maximum(y / y.max(), 1e-4)), -4.0)
        dyes.append(np.where((WAVELENGTHS >= 400.0) & (WAVELENGTHS <= top), 10.0 ** logged, np.nan))
    dyes = unit(np.array(dyes).T, STATUS_A)
    crosstalk = np.array([read(dyes[:, k], STATUS_A) for k in range(3)]).T
    measured.append(f"cyan, magenta and yellow dye shapes ({dye_chart['confidence']} confidence)")
    inferred.append("each dye past its sheet's points (hidden under another curve), falling at its end slope in log")
    inferred.append("each dye's unit: a Status A density of 1 in its own channel, by ISO 5-3's Status A products")
    inferred.append("the crosstalk: the Status A densities each dye reads at its unit amount, a row a channel, a column a dye")
    curves_chart = chart(stock, "characteristic_curves")
    raw = {k: series(curves_chart, k) for k in "RGB"}
    le = np.linspace(min(v[0][0] for v in raw.values()), min(v[0][-1] for v in raw.values()), CURVE_NODES)
    curves = {}
    for k, (cx, cy) in raw.items():
        others = [np.interp(le, *raw[o]) for o in "RGB" if o != k and raw[o][0][0] <= cx[0]]
        hidden = le < cx[0]
        if hidden.any():
            inferred.append(f"the {k} curve under its first printed point, hidden on the sheet under the others: their mean")
        curves[k] = np.where(hidden, np.mean(others, axis=0) if others else cy[0], np.interp(le, cx, cy))
    channels = {k: np.maximum.accumulate(curves[k]) for k in "RGB"}
    # The axis' origin where green prints an 18 % grey over its D-min, so the enlarger's solve starts on the curves' slope.
    origin = float(np.interp(-math.log10(0.18), channels["G"] - channels["G"].min(), le))
    le = le - origin
    inferred.append("the curves' log H origin: where green prints an 18 % grey over its D-min; the enlarger's time absorbs it")
    sheet = np.array([channels[k] for k in "RGB"]).T
    measured.append(f"characteristic curves, Status A, as printed ({curves_chart['confidence']} confidence), each on its D-min")
    inferred.append("densities held at the sheet's last point past the curves' printed range")
    name = stock_id.removesuffix("_paper").removesuffix("_printing_material")
    source = stock["source"]
    profile = {
        "info": {"stock": name, "name": stock["name"], "manufacturer": stock["manufacturer"], "type": "paper", "densitometer": "status_A"},
        "provenance": {
            "data_sheet": source.get("doc_id"),
            "url": source.get("url"),
            "sha256": source.get("sha256"),
            "retrieved": source.get("retrieved"),
            "dataset_id": stock_id,
            "charts": [dye_chart["chart_id"], curves_chart["chart_id"]],
            "measured": measured,
            "inferred": inferred,
        },
        "data": {
            "log_exposure": [round(float(v), 6) for v in le],
            "density_curves": [[round(float(v), 6) for v in row] for row in sheet],
            "crosstalk": [[round(float(v), 6) for v in row] for row in crosstalk],
        },
    }
    place(profile)
    return name, inferred


# The papers whose gaze reads a film: the dataset's id, the paper's key and, for a paper with no file yet, its name. A
# sheet printing no characteristic curve gives a file of the sensitivities alone, which reads films and prints nothing.
READERS = [
    ("kodak_endura_premier_paper", "kodak-endura-premier", None),
    ("kodak_ektacolor_edge_paper", "kodak-ektacolor-edge", None),
    ("fujifilm_crystal_archive_paper_super_type_p", "fujifilm-crystal-archive-super-type-p", "Fujifilm Crystal Archive Super Type P"),
    ("fujifilm_crystal_archive_paper_supreme", "fujifilm-crystal-archive-supreme", "Fujifilm Crystal Archive Supreme"),
]

# A still colour negative's paper where its maker's sheet names one: "optimized for printing to" ENDURA Premier (Portra
# 160, 400, 800, Ektar 100), "optimized for printing on" EDGE first (Gold 200, UltraMax 400).
SHEET_PAPERS = {
    "kodak-portra160": "kodak-endura-premier", "kodak-portra400": "kodak-endura-premier", "kodak-portra800": "kodak-endura-premier",
    "kodak-ektar100": "kodak-endura-premier", "kodak-gold200": "kodak-ektacolor-edge", "kodak-ultramax400": "kodak-ektacolor-edge",
}
# A Fujifilm professional stock's file, by its key less the brand: the Pros, the 160N line, New Pro 400 and Reala Ace.
FUJI_PROFESSIONAL = ("pro", "160n", "new-pro400", "reala-ace")


def paper_for(file_key):
    """The paper whose gaze reads the stock of `file_key`, and where the pairing comes from: its maker's sheet, else
    the owner's choice (2026-11-25), no sheet of the dataset naming a paper for it."""
    if file_key in SHEET_PAPERS:
        return SHEET_PAPERS[file_key], "the paper the film's own data sheet names for printing it"
    if file_key.startswith("kodak"):
        return "kodak-endura-premier", "the owner's choice: the papers its sheet names (Supra, Ultra Endura) print no spectral sensitivity"
    rest = file_key.removeprefix("fujifilm-")
    if rest.startswith(FUJI_PROFESSIONAL[:2]) or rest in FUJI_PROFESSIONAL[2:]:
        return "fujifilm-crystal-archive-super-type-p", "the owner's choice for a Fujifilm professional film: its sheet names no paper"
    return "fujifilm-crystal-archive-supreme", "the owner's choice for a Fujifilm consumer film: its sheet names no paper"


def convert_reader(stock_id, key, name):
    """A paper's three layers' spectral sensitivities on the grid, laid as the `sensitivity` of its file: what its gaze
    on a film's dyes is computed from, under an enlarger's lamp."""
    stock = STOCK[stock_id]
    log_sensitivity, c = sensitivity(stock)
    source = stock["source"]
    block = {
        "provenance": {
            "data_sheet": source.get("doc_id"),
            "url": source.get("url"),
            "sha256": source.get("sha256"),
            "retrieved": source.get("retrieved"),
            "dataset_id": stock_id,
            "charts": [c["chart_id"]],
            "measured": [f"spectral sensitivity, three layers, as printed ({c['confidence']} confidence)"],
            "inferred": ["each layer's sensitivity past the sheet's first and last point, falling at its end slope"],
        },
        "data": {
            "wavelengths": [float(w) for w in WAVELENGTHS],
            "log_sensitivity": [[round(float(v), 6) for v in row] for row in log_sensitivity],
        },
    }
    path = STOCKS_DIR / f"{key}.json"
    if path.exists():
        paper = json.loads(path.read_text())
    else:
        paper = {"oni": "paper", "format": 1, "key": key, "name": name, "brand": name.split(" ")[0]}
        print(f"{key}.json: a new file, a paper reading films and printing nothing")
    if paper.get("sensitivity") != block:
        paper["sensitivity"] = block
        path.write_text(json.dumps(paper, indent=1, ensure_ascii=False) + "\n")
    return key


def pair_papers():
    """Each still colour negative whose file holds its sheet's profile is read by its paper's gaze, its dyes uncoupled:
    its reference and its filtration are then derived again."""
    uncoupled = [[0.0, 0.0, 0.0] for _ in range(3)]
    for path in stock_files():
        stock_file = json.loads(path.read_text())
        if stock_file.get("oni") != "film" or stock_file.get("type") != "colour-negative":
            continue
        moved = False
        for block in blocks(stock_file):
            if block.get("neutral") or "channel_density" not in block["profile"].get("data", {}):
                continue
            paper, why = paper_for(stock_file["key"])
            if (block.get("read_by"), block.get("couplers"), block["profile"]["provenance"].get("read_by")) == (paper, uncoupled, why) and "filtration" in block:
                continue
            laid = {}
            for k, v in block.items():
                if k in ("reference", "filtration", "read_by", "couplers"):
                    continue
                laid[k] = v
                if k == "dyes":
                    laid["read_by"] = paper
                    laid["couplers"] = uncoupled
            laid["profile"]["provenance"]["read_by"] = why
            block.clear()
            block.update(laid)
            moved = True
        if moved:
            path.write_text(json.dumps(stock_file, indent=1, ensure_ascii=False) + "\n")
            MOVED.append(path)
            print(f"{path.name}: read by {paper}")


# The colour reversal films taken: the dataset's id, the stock file whose virtual film it becomes, that film's key, and
# each curve the sheet hides under another's stroke (coincident within line width, the dataset's notes say) with that one.
SLIDES = [
    ("fujifilm_velvia_50", "fujifilm-velvia50", "fujifilm-velvia-50", {}),
    ("fujifilm_velvia_100", "fujifilm-velvia100", "fujifilm-velvia-100", {}),
    ("fujifilm_provia_100f", "fujifilm-provia100", "fujifilm-provia-100f", {"G": "R", "B": "R"}),
    ("kodak_ektachrome_e100", "kodak-ektachrome", "kodak-ektachrome-e100", {}),
]

CHANNEL_NAMES = {"R": "red", "G": "green", "B": "blue"}


def convert_slide(stock_id, file_key, film_key, hidden):
    """A colour reversal film's profile, laid as the virtual film of its stock's file: a paper's parts, Status A, the
    dyes unmixed over the base, the layers' sensitivity as printed. Its curves fall with exposure: each held non-rising,
    the dyes unmixed from the densest end, the log H origin where green transmits an 18 % grey over the base. A curve
    `hidden` under another's stroke past its last traced point takes that one's points there."""
    stock = STOCK[stock_id]
    measured, inferred = [], []
    sens_chart = chart(stock, "spectral_sensitivity")
    layers = []
    for channel in ("red_sensitive", "green_sensitive", "blue_sensitive"):
        x, y = series(sens_chart, channel)
        layers.append(tailed(x, y, y.max() - 8.0))
    measured.append(f"spectral sensitivity, three layers, as printed ({sens_chart['confidence']} confidence)")
    inferred.append("each layer's sensitivity past the sheet's first and last point, falling at its end slope")
    dye_chart = chart(stock, "spectral_dye_density")
    top = max(series(dye_chart, ch)[0][-1] for ch in ("cyan", "magenta", "yellow"))
    dyes = []
    for channel in ("cyan", "magenta", "yellow"):
        x, y = series(dye_chart, channel)
        logged = tailed(x, np.log10(np.maximum(y / y.max(), 1e-4)), -4.0)
        dyes.append(np.where((WAVELENGTHS >= 400.0) & (WAVELENGTHS <= top), 10.0 ** logged, np.nan))
    channel = unit(np.array(dyes).T, STATUS_A)
    measured.append(f"cyan, magenta and yellow dye shapes ({dye_chart['confidence']} confidence)")
    inferred.append("each dye's unit: a Status A density of 1 in its own channel, by ISO 5-3's Status A products")
    curves_chart = chart(stock, "characteristic_curves")
    raw = {k: series(curves_chart, k) for k in "RGB"}
    for k, under in hidden.items():
        (x, y), (ux, uy) = raw[k], raw[under]
        past = ux > x[-1]
        raw[k] = (np.concatenate([x, ux[past]]), np.concatenate([y, uy[past]]))
        inferred.append(f"{CHANNEL_NAMES[k]}'s curve past log H {x[-1]:.2f}, hidden on the sheet under {CHANNEL_NAMES[under]}'s stroke: {CHANNEL_NAMES[under]}'s points")
    le = np.linspace(max(v[0][0] for v in raw.values()), min(v[0][-1] for v in raw.values()), CURVE_NODES)
    curves = {k: np.interp(le, cx, cy) for k, (cx, cy) in raw.items()}
    lows = [curves[k].min() for k in "RGB"]
    channels = {k: np.minimum.accumulate(curves[k]) - curves[k].min() for k in "RGB"}
    le = le - float(np.interp(-math.log10(0.18), channels["G"][::-1], le[::-1]))
    inferred.append("the curves' log H origin: where green transmits an 18 % grey over the base")
    bare = np.where(np.isnan(channel).any(axis=1), np.nan, 0.0)
    base = channel @ unmixed([lows], bare, channel, STATUS_A)[0]
    sheet = np.array([channels[k] for k in "RGB"]).T
    amounts_ = unmixed(sheet[::-1], base, channel, STATUS_A)[::-1]
    measured.append(f"characteristic curves, Status A, as printed ({curves_chart['confidence']} confidence), each less its D-min")
    inferred.append("the dyes' amounts along log H: the curves unmixed from the densest end, as they fall")
    source = stock["source"]
    profile = {
        "info": {"stock": stock_id, "name": stock["name"], "manufacturer": stock["manufacturer"], "type": "reversal",
                 "densitometer": "status_A", "reference_illuminant": "D"},
        "provenance": {
            "data_sheet": source.get("doc_id"),
            "url": source.get("url"),
            "sha256": source.get("sha256"),
            "retrieved": source.get("retrieved"),
            "dataset_id": stock_id,
            "charts": [c["chart_id"] for c in (sens_chart, dye_chart, curves_chart)],
            "measured": measured,
            "inferred": inferred,
        },
        "data": {
            "wavelengths": [float(w) for w in WAVELENGTHS],
            "log_sensitivity": [[round(float(v), 6) for v in row] for row in np.array(layers).T],
            "channel_density": [nulls(row) for row in channel],
            "base_density": nulls(base),
            "log_exposure": [round(float(v), 6) for v in le],
            "density_curves": [[round(float(v), 6) for v in row] for row in amounts_],
            "sheet_density_curves": [[round(float(v), 6) for v in row] for row in sheet],
        },
    }
    path = STOCKS_DIR / f"{file_key}.json"
    stock_file = json.loads(path.read_text())
    # Nothing reads a slide as a negative: it rests on the generic set, as a black and white film does.
    film = stock_file.get("film") or {"key": film_key, "balance": "daylight", "dyes": "generic-c41"}
    if film.get("profile") != profile:
        couplers = film.get("couplers")
        film = {"key": film["key"], "balance": film["balance"], "dyes": film["dyes"], **({"couplers": couplers} if couplers else {}), "profile": profile}
        stock_file["film"] = film
        path.write_text(json.dumps(stock_file, indent=1, ensure_ascii=False) + "\n")
        MOVED.append(path)
    return file_key, inferred


# The black and white negatives taken: the dataset's id, the file ONI names, the chart-id prefix of the developer the
# stock rests on (the sheet's own, D-76 where the sheet prints it), and the stock whose spectrum stands for a missing one.
BW_STOCKS = [
    ("foma_fomapan_100_classic", "foma_fomapan_100", "characteristic_curves_dev", None),
    ("foma_fomapan_200_creative", "foma_fomapan_200", "characteristic_curves_dev", None),
    ("foma_fomapan_400_action", "foma_fomapan_400", "characteristic_curves_dev", None),
    ("foma_fomapan_cine_100", "foma_fomapan_cine_100", "characteristic_curves_dev", None),
    ("foma_fomapan_cine_400", "foma_fomapan_cine_400", "characteristic_curves_dev", None),
    ("foma_ortho_400", "foma_ortho_400", "characteristic_curves_dev", None),
    ("foma_cine_ortho_400", "foma_cine_ortho_400", "characteristic_curves_dev", None),
    ("foma_retropan_320_soft", "foma_retropan_320", "characteristic_curves_dev", None),
    ("fujifilm_neopan_100_acros_ii", "fujifilm_acros_ii", "characteristic_curves_d76", None),
    ("fujifilm_neopan_400_presto", "fujifilm_neopan_400", "characteristic_curves_d76", None),
    ("fujifilm_neopan_1600_super_presto", "fujifilm_neopan_1600", "characteristic_curves_d76", None),
    ("fujifilm_neopan_ortho", "fujifilm_neopan_ortho", "characteristic_curves_artdol", None),
    ("ilford_delta_100", "ilford_delta_100", "characteristic_curves", None),
    ("ilford_delta_400", "ilford_delta_400", "characteristic_curves", None),
    ("ilford_delta_3200", "ilford_delta_3200", "characteristic_curves_ddx", None),
    ("ilford_fp4_plus", "ilford_fp4_plus", "characteristic_curves", None),
    ("ilford_hp5_plus", "ilford_hp5_plus", "characteristic_curves", None),
    ("ilford_ortho_plus", "ilford_ortho_plus", "characteristic_curves_id11", None),
    ("ilford_pan_f_plus", "ilford_pan_f_plus", "characteristic_curves", None),
    ("ilford_sfx_200", "ilford_sfx_200", "characteristic_curves", None),
    ("ilford_xp2_super", "ilford_xp2_super", "characteristic_curves", None),
    ("kodak_eastman_double_x_negative_5222", "kodak_double_x", "characteristic_curves_dev", None),
    ("kodak_t_max_100", "kodak_t_max_100", "characteristic_curves_d76", None),
    ("kodak_t_max_400", "kodak_t_max_400", "characteristic_curves_d76", None),
    ("kodak_t_max_p3200", "kodak_t_max_p3200", "characteristic_curves_tmax_dev_small_tank", None),
    ("kodak_tri_x_320", "kodak_tri_x_320", "characteristic_curves_hc110b", "kodak_tri_x_400"),
    ("kodak_tri_x_400", "kodak_tri_x_400", "characteristic_curves_d76_35mm", None),
]

# The black and white negatives left out, each with why.
BW_LEFT_OUT = {
    "adox_hr_50": "its curve's log exposure axis prints no scale",
    "fujifilm_neopan_ss": "its toe is not traced: its five curves merge there and each trace jumps 0.13 density at log H -2.10",
    "rollei_infrared_400": "no curve and no spectrum on its sheet",
    "rollei_retro_400s": "no curve and no spectrum on its sheet",
    "rollei_retro_80s": "no characteristic curve on its sheet",
    "rollei_rpx_25": "no characteristic curve on its sheet",
    "rollei_rpx_100": "no characteristic curve on its sheet",
    "rollei_rpx_400": "no characteristic curve on its sheet",
    "rollei_superpan_200": "no characteristic curve on its sheet",
    "rollei_ortho_25_plus": "no characteristic curve on its sheet",
}

# ISO 6: the speed point 0.10 over base and fog, and the development whose density climbs 0.80 over the 1.30 log H past it.
ISO6_RISE, ISO6_SPAN, ISO6_GRADIENT = 0.10, 1.30, 0.80 / 1.30
# ISO 6's speed 0.8 / H_m and ISO 2720's meter (K 12.5, q 0.65) set a metered grey 1.01 log over the speed point.
ISO6_GREY = math.log10(0.65 * 12.5 / 0.8)


# A raster trace climbs in steps of its pixels, a few hundredths of log H wide: a curve is smoothed over 0.05 log H
# (one sigma, a sixth of a stop), well under its toe's and shoulder's bends.
SMOOTHING, FINE = 0.05, 0.005


def neutral(chart_):
    """A black and white curve's density on a fine log H grid, smoothed, held at its ends and at a point already
    reached where the trace dips."""
    x, y = series(chart_, "neutral")
    fine = np.arange(x[0], x[-1] + FINE / 2, FINE)
    taps = np.arange(-4 * SMOOTHING, 4 * SMOOTHING + FINE / 2, FINE)
    w = np.exp(-0.5 * (taps / SMOOTHING) ** 2)
    held = np.pad(np.interp(fine, x, y), len(taps) // 2, mode="edge")
    return fine, np.maximum.accumulate(np.convolve(held, w / w.sum(), mode="valid"))


def iso6(chart_):
    """The curve's ISO 6 speed point in log H and its average gradient from there over 1.30 log H."""
    x, y = neutral(chart_)
    target = y.min() + ISO6_RISE
    i = int(np.argmax(y >= target))
    speed = float(np.interp(target, y[: i + 1], x[: i + 1]))
    return speed, (float(np.interp(speed + ISO6_SPAN, x, y)) - target) / ISO6_SPAN


def development(stock, prefix):
    """The developer's curves traced from their foot (from within 0.10 log H of the first: a curve whose toe hides
    under another's is traced from where they part), and among them the one nearest ISO 6's gradient."""
    charts = [c for c in stock["charts"] if c["kind"] == "characteristic_curves" and c["chart_id"].startswith(prefix)]
    first = min(neutral(c)[0][0] for c in charts)
    traced = [c for c in charts if neutral(c)[0][0] <= first + 0.10]
    return min(traced, key=lambda c: abs(iso6(c)[1] - ISO6_GRADIENT))


def bw_sensitivity(stock):
    """The one layer's log sensitivity on the grid, its sheet's gaps filled, and its chart; Kodak's at the 0.3
    criterion where the sheet prints two."""
    charts = [c for c in stock["charts"] if c["kind"] == "spectral_sensitivity"]
    c = next((c for c in charts if c["chart_id"].endswith(("_0p3", "_d03"))), charts[0])
    channel = next(ch for ch in ("panchromatic", "orthochromatic") if ch in c["channels"])
    x, y = series(c, channel)
    return tailed(x, y, y.max() - 8.0), c, channel


def convert_bw(stock_id, name, prefix, spectrum_of):
    stock = STOCK[stock_id]
    measured, inferred = [], []
    layer, sens_chart, channel = bw_sensitivity(STOCK[spectrum_of or stock_id])
    unit = sens_chart["y"]["unit"]
    measured.append(f"spectral sensitivity, one {channel} layer, as printed ({sens_chart['confidence']} confidence)")
    if spectrum_of:
        inferred.append(f"the spectral sensitivity: {STOCK[spectrum_of]['name']}'s, which the same sheet prints for both")
    if not unit.startswith("log10"):
        inferred.append("the sensitivity's printed height read as log sensitivity, one unit of its axis a log: a wedge "
                        "spectrogram's envelope stands on log exposure")
    inferred.append("the layer's sensitivity past the sheet's first and last point, falling at its end slope")
    inferred.append("the layer's level: set by the bake so a grey under daylight exposes it at log 0")

    chosen = development(stock, prefix)
    speed, gradient = iso6(chosen)
    grey = speed + ISO6_GREY
    x, y = neutral(chosen)
    le = np.linspace(x[0], x[-1], CURVE_NODES)
    density = np.interp(le, x, y) - y.min()
    measured.append(f"characteristic curve, visual density, as printed ({chosen['confidence']} confidence), less its base and fog")
    inferred.append(f"the curve smoothed over {SMOOTHING:.2f} log H, lifting a raster trace's steps")
    inferred.append(f"the development: {chosen['chart_id'].removeprefix('characteristic_curves_') or 'the sheet\'s one curve'}, "
                    f"its ISO 6 average gradient {gradient:.2f} the nearest the sheet's developer prints to ISO 6's 0.62")
    inferred.append(f"the grey's exposure: ISO 6 speed point + {ISO6_GREY:.2f} log, where ISO 6's speed and ISO 2720's meter put a metered grey")
    inferred.append("densities held at the sheet's last point past the curve's printed range")

    others = []
    for c in stock["charts"]:
        if c["kind"] != "characteristic_curves" or c is chosen:
            continue
        cx, cy = neutral(c)
        node = np.linspace(cx[0], cx[-1], CURVE_NODES)
        others.append({
            "chart": c["chart_id"],
            "condition": c["condition"],
            "average_gradient": round(iso6(c)[1], 4),
            "log_exposure": [round(float(v), 5) for v in node - grey],
            "density_curve": [round(float(v), 5) for v in np.interp(node, cx, cy) - cy.min()],
        })
    if others:
        inferred.append("every other development on the default's log H axis, less its grey: one sheet's axis is shared")
    source = stock["source"]
    profile = {
        "info": {
            "stock": name,
            "name": stock["name"],
            "manufacturer": stock["manufacturer"],
            "type": "bw_negative",
            "densitometer": "visual",
            "sensitisation": channel,
            "reference_illuminant": "D55",
        },
        "provenance": {
            "data_sheet": source.get("doc_id"),
            "url": source.get("url"),
            "sha256": source.get("sha256"),
            "retrieved": source.get("retrieved"),
            "dataset_id": stock_id,
            "charts": [sens_chart["chart_id"], chosen["chart_id"]],
            "measured": measured,
            "inferred": inferred,
        },
        "data": {
            "wavelengths": [float(w) for w in WAVELENGTHS],
            "log_sensitivity": [round(float(v), 6) for v in layer],
            "development": {"chart": chosen["chart_id"], "condition": chosen["condition"], "average_gradient": round(gradient, 4)},
            "log_exposure": [round(float(v), 6) for v in le - grey],
            "density_curve": [round(float(v), 6) for v in density],
        },
    }
    place(profile)
    return name, inferred, len(others)


def stock_files():
    """The folder's stock and paper files, hidden ones left out as the build leaves them."""
    return sorted(p for p in STOCKS_DIR.glob("*.json") if not p.name.startswith("."))


def blocks(stock_file):
    """Every profile a stock's or a paper's file holds, with the block holding it."""
    if stock_file.get("oni") == "paper":
        return [stock_file] if "profile" in stock_file else []
    films = [stock_file.get("film")] + [d.get("film") for d in stock_file.get("developments", [])]
    return [f for f in films if f]


MOVED = []


def place(profile):
    """`profile` laid into the file holding the profile of its name, every other field kept; else a file of its own."""
    name = profile["info"]["stock"]
    for path in stock_files():
        stock_file = json.loads(path.read_text())
        for block in blocks(stock_file):
            if block["profile"]["info"]["stock"] != name:
                continue
            # Where its reading paper comes from is laid by the pairing, kept across a profile written again.
            if "read_by" in block["profile"].get("provenance", {}):
                profile["provenance"]["read_by"] = block["profile"]["provenance"]["read_by"]
            if block["profile"] != profile:
                block["profile"] = profile
                if stock_file["oni"] == "film":
                    block.pop("reference", None)
                    block.pop("filtration", None)
                    MOVED.append(path)
                path.write_text(json.dumps(stock_file, indent=1, ensure_ascii=False) + "\n")
            return
    key = name.replace("_", "-")
    brand = profile["info"]["manufacturer"].split(" ")[0]
    if profile["info"]["type"] == "paper":
        new = {"oni": "paper", "format": 1, "key": key, "name": profile["info"]["name"], "brand": brand, "profile": profile}
    else:
        silver = profile["info"]["type"] == "bw_negative"
        film = {"key": key, "balance": "tungsten" if profile["info"]["reference_illuminant"] == "T" else "daylight",
                "dyes": "generic-c41" if silver else key, "profile": profile}
        new = {"oni": "film", "format": 1, "key": key, "name": profile["info"]["name"], "brand": brand,
               "type": "black-and-white" if silver else "colour-negative", "film": film}
        if not silver:
            new["dyes"] = key
        MOVED.append(STOCKS_DIR / f"{key}.json")
    (STOCKS_DIR / f"{key}.json").write_text(json.dumps(new, indent=1, ensure_ascii=False) + "\n")
    print(f"{key}.json: a new file, its name, brand, speed and canister to check")


def float32(v):
    """The shortest decimal of a single, which reads back as that single."""
    return float(str(np.float32(v)))


def refresh_references():
    """Each film whose profile or paper moved takes the reference, and the filtration where a paper reads it, that the
    pipeline derives from its sheets, laid beside its dyes."""
    moved = sorted(set(MOVED))
    if not moved:
        return
    out = subprocess.run(["cargo", "run", "-q", "-p", "oni-pipeline", "--example", "stock_references", "--", *map(str, moved)],
                         cwd=ROOT, check=True, capture_output=True, text=True).stdout
    derived = json.loads(out)
    for path in moved:
        stock_file = json.loads(path.read_text())
        for block in blocks(stock_file):
            reference = {k: [float32(v) for v in derived[path.name][block["key"]][k]] for k in ("stops", "black", "white")}
            filtration = derived[path.name][block["key"]]["filtration"]
            laid = {}
            for k, v in block.items():
                if k in ("reference", "filtration"):
                    continue
                laid[k] = v
                if k == "dyes":
                    laid["reference"] = reference
                    if filtration is not None:
                        laid["filtration"] = [round(v, 5) for v in filtration]
            block.clear()
            block.update(laid)
        path.write_text(json.dumps(stock_file, indent=1, ensure_ascii=False) + "\n")
        print(f"{path.name}: its reference derived again")


# A film's halation by the level photographers report for it (`scripts/halation-levels.json`, sources kept), the share of a
# clipped light's exposure its glow adds: medium lays Portra 800's glow past a lamp, low a third of it, high three times.
HALATION = {"none": 0.0, "low": 0.10, "medium": 0.31, "high": 0.92}
# The glow's deviation on the film, in micrometres: Portra 800's red halo past a lamp falls as this Gaussian's from 150 to
# 450 µm.
HALATION_WIDTH = 200.0
LEVELS = json.loads((Path(__file__).resolve().parent / "halation-levels.json").read_text())


def sheet_of(profile_name):
    """The dataset's stock a profile's name was converted from."""
    names = {file_name(i, push): i for i, push in STOCKS}
    names.update({i: i for i, *_ in SLIDES})
    names.update({name: i for i, name, *_ in BW_STOCKS})
    return STOCK.get(names.get(profile_name, profile_name))


def bloom(stock):
    """The emulsion's blur, a Gaussian's deviation in micrometres: the one whose response meets the sheet's MTF where it
    first falls to a half, the green layer's on a colour film, or at the chart's last frequency where it never does."""
    c = chart(stock, "mtf")
    if c is None:
        return None
    channel = "G" if "G" in c["channels"] else next(iter(c["channels"]))
    f, r = series(c, channel)
    r = r / (100.0 if c["y"]["unit"] == "%" else 1.0)
    below = np.nonzero(r <= 0.5)[0]
    if len(below) and below[0] > 0:
        i = below[0]
        at = f[i - 1] + (f[i] - f[i - 1]) * (r[i - 1] - 0.5) / (r[i - 1] - r[i])
        level = 0.5
    else:
        at, level = f[-1], r[-1]
    if not 0.0 < level < 1.0:
        return None
    return round(1000.0 * math.sqrt(-math.log(level) / (2.0 * math.pi**2)) / at, 1)


def lay_effects():
    """Each film's own effects laid before its profile: its halation by the level reported for it, its emulsion's blur
    from its sheet's MTF."""
    for path in stock_files():
        stock_file = json.loads(path.read_text())
        if stock_file.get("oni") != "film":
            continue
        changed = False
        for block in blocks(stock_file):
            if block.get("neutral"):
                continue
            level = LEVELS[stock_file["key"]]["level"]
            effects = {"halation": {"dose": HALATION[level], "size": HALATION_WIDTH}}
            sheet = sheet_of(block["profile"]["info"]["stock"])
            soft = bloom(sheet) if sheet is not None else None
            if soft is not None:
                effects["bloom"] = soft
            print(f"{block['key']}: halation {level}; bloom {soft}")
            if block.get("effects") != effects:
                laid = {}
                for k, v in block.items():
                    if k == "effects":
                        continue
                    if k == "profile":
                        laid["effects"] = effects
                    laid[k] = v
                block.clear()
                block.update(laid)
                changed = True
        if changed:
            path.write_text(json.dumps(stock_file, indent=1, ensure_ascii=False) + "\n")


# The colour negatives whose sheets give the neutral film its tone curve: the photographers' common Kodak stocks.
NEUTRAL_SOURCES = ["kodak_portra_160", "kodak_portra_400", "kodak_portra_800", "kodak_ektar_100", "kodak_gold_200",
                   "kodak_ultra_max_400"]
NEUTRAL_NODES = 64


def neutral_curve():
    """The neutral film's tone curve: the mean, over its sources' three layers, of their sheets' density above D-min on log
    H about a grey, on the span they all cover, nought at its foot; its own colour none, the same on its three layers."""
    curves = []
    for path in stock_files():
        stock_file = json.loads(path.read_text())
        for block in blocks(stock_file):
            if stock_file.get("oni") == "film" and block["profile"]["info"]["stock"] in NEUTRAL_SOURCES:
                data = block["profile"]["data"]
                curves.append((np.array(data["log_exposure"]), np.array(data["sheet_density_curves"], dtype=float)))
    assert len(curves) == len(NEUTRAL_SOURCES), f"{len(curves)} sources found"
    low, high = max(le[0] for le, _ in curves), min(le[-1] for le, _ in curves)
    grid = np.linspace(low, high, NEUTRAL_NODES)
    mean = np.mean([np.interp(grid, le, d[:, c]) for le, d in curves for c in range(3)], axis=0)
    mean -= mean[0]
    path = STOCKS_DIR / "generic-neutral.json"
    stock_file = json.loads(path.read_text())
    data = stock_file["film"]["profile"]["data"]
    data["log_exposure"] = [round(float(v), 6) for v in grid]
    data["density"] = [round(float(v), 6) for v in mean]
    stock_file["film"]["profile"]["provenance"] = {"density": "the mean of " + ", ".join(NEUTRAL_SOURCES) + "'s sheet curves, their three layers each"}
    path.write_text(json.dumps(stock_file, indent=1, ensure_ascii=False) + "\n")
    print(f"neutral: the mean of {len(curves)} sheets' layers on log H {low:+.2f} to {high:+.2f}, {mean[-1]:.3f} at its top")


def main():
    global STOCK
    dataset = Path(sys.argv[1])
    STOCK = {s["id"]: s for s in json.loads((dataset / "export/filmstocks.json").read_text())["stocks"]}
    gold = chart(STOCK[REFERENCE_SHEET], "characteristic_curves", "characteristic_curves")
    offset = gold["log_h_ref"] - speed_point(gold)
    basis = vision3_basis()
    names = []
    for stock_id, push in STOCKS:
        name, inferred = convert(stock_id, push, offset, basis)
        names.append(name)
        print(f"{name}: " + "; ".join(inferred[-2:-1]))
    for stock_id in PAPERS:
        name, inferred = convert_paper(stock_id)
        names.append(name)
        print(f"{name}: " + "; ".join(inferred))
    for stock_id, key, name in READERS:
        convert_reader(stock_id, key, name)
    for stock_id, file_key, film_key, hidden in SLIDES:
        name, inferred = convert_slide(stock_id, file_key, film_key, hidden)
        names.append(name)
        print(f"{name}: " + "; ".join(inferred[-2:]))
    for stock_id, name, prefix, spectrum_of in BW_STOCKS:
        name, inferred, others = convert_bw(stock_id, name, prefix, spectrum_of)
        names.append(name)
        print(f"{name}: " + "; ".join(i for i in inferred if i.startswith(("the development", "the spectral", "the sensitivity's")))
              + f"; {others} other development{'s' if others != 1 else ''} left out")
    taken = {s for s, *_ in BW_STOCKS}
    for s in STOCK.values():
        if s["type"] == "bw_negative" and s["id"] not in taken:
            print(f"left out: {s['id']}: {BW_LEFT_OUT[s['id']]}")
    pair_papers()
    refresh_references()
    lay_effects()
    neutral_curve()


if __name__ == "__main__":
    main()
