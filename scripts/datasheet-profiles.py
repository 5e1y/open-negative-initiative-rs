#!/usr/bin/env python3
"""Colour negative film profiles from the makers' data sheets, in the shape ONI's bake reads.

Reads the emulsion dataset's export (filmstocks.json: curves traced from each official data sheet) and writes one
profile per stock into crates/oni-pipeline/resources/film/datasheet/, each with its sheet's reference, URL and
SHA-256 and a list of what is measured and what is inferred, plus crates/oni-pipeline/src/film/datasheet.rs,
the table the bake includes. Run: scripts/datasheet-profiles.py <emulsion-dataset folder>. Needs numpy.

What each part of a profile is:
- log_sensitivity: the sheet's three layers on 380-780 nm, linear in log between its points; past the first and
  last point each layer falls on at the slope of its last 15 nm, at least 0.02 log per nm (the sheet stops where
  the curve leaves its chart). Their levels are left as printed: the bake scales each layer so a grey under the
  stock's light exposes it at log 0, which is what the characteristic curves' common log H axis states.
- density_curves: the sheet's Status M curves less each channel's least density, on log H less the grey's; the
  grey sits 1.03 log above the ISO 5800 speed point of the curves themselves, the place Kodak's one printed
  "Log H Ref" (Gold 200, E-7022) holds over that sheet's speed point; a push sits n x 0.30 log under its stock's.
- channel_density: the dyes. A Vision3 sheet prints its cyan, magenta and yellow: their shapes are kept and their
  amounts fitted to the printed mid-scale neutral. Every other sheet prints only the mid-scale neutral and D-min:
  the Vision3 sheets' mean dyes are shifted and widened per dye (within 40 nm and 0.8-1.25, each held at its
  sheet's last value for 40 nm past it) to fit that neutral
  less D-min. Each dye is then scaled to a Status M density of 1 in its own channel, the curves' unit; Status M
  is approximated by normal bands at 650, 535 and 440 nm, 35, 40 and 40 nm wide at half height.
- base_density: the sheet's D-min, printed.

A colour paper's profile holds the same parts, its densities Status A (approximated by normal bands at 620, 540 and
440 nm, 30, 40 and 40 nm wide); only a paper whose sheet prints its characteristic curves is taken, and a display
material viewed by transmission is written as a paper, its densities read as its transmittance under the viewing
light. A sheet prints no base spectrum: the base is the paper's own dyes in the amounts that read the curves' least
densities. Each layer's level is as printed: Kodak's absolute, Fujifilm's on one relative axis for all three; the
curves' log H origin is set where green prints an 18 % grey, the enlarger's time absorbing it. A dye's or a layer's
points hidden on its sheet are filled by its end slope.
"""

import json
import math
import sys
from pathlib import Path

import numpy as np

WAVELENGTHS = np.arange(380.0, 781.0, 5.0)
CURVE_NODES = 256
ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/oni-pipeline/resources/film/datasheet"
TABLE = ROOT / "crates/oni-pipeline/src/film/datasheet.rs"
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


# Status M's red, green and blue bands, approximated: centre and full width at half height, in nm.
STATUS_M = [(650.0, 35.0), (535.0, 40.0), (440.0, 40.0)]


def status_m(dye, channel):
    """A dye's density as Status M's band `channel` reads it, on the wavelengths the dye is known."""
    centre, width = STATUS_M[channel]
    w = np.exp(-0.5 * ((WAVELENGTHS - centre) / (width / 2.3548)) ** 2)
    ok = ~np.isnan(dye)
    return -math.log10(float((10.0 ** -dye[ok] * w[ok]).sum() / w[ok].sum()))


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
    channel = dyes / np.array([status_m(dyes[:, c], c) for c in range(3)])
    inferred.append("each dye's unit: a Status M density of 1 in its own channel, Status M approximated by normal bands")
    band = ~np.isnan(base) & ~np.isnan(channel).any(axis=1)
    channel[~band] = np.nan
    base = np.where(band, base, np.nan)

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
            "density_curves": [[round(float(v), 6) for v in row] for row in density],
        },
    }
    (OUT / f"{name}.json").write_text(json.dumps(profile, indent=1) + "\n")
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
    "fujifilm_fujiclear_display_material_for_digital_printers",
]

# Status A's red, green and blue bands, approximated: centre and full width at half height, in nm.
STATUS_A = [(620.0, 30.0), (540.0, 40.0), (440.0, 40.0)]


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
    stock = STOCK[stock_id]
    measured, inferred = [], []
    # A sheet edition printing the curves may print its sensitivities on a log axis the current one leaves unstated.
    sens_chart = next((c for c in stock["charts"] if c["kind"] == "spectral_sensitivity" and "log10" in c["y"]["unit"]), None)
    if sens_chart is None:
        raise SystemExit(f"{stock_id}: no sensitivity on a stated log axis")
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
    dyes = np.array(dyes).T
    channel = dyes / np.array([status_a(dyes[:, c], c) for c in range(3)])
    measured.append(f"cyan, magenta and yellow dye shapes ({dye_chart['confidence']} confidence)")
    inferred.append("each dye past its sheet's points (hidden under another curve), falling at its end slope in log")
    inferred.append("each dye's unit: a Status A density of 1 in its own channel, Status A approximated by normal bands")

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
    lows = [curves[k].min() for k in "RGB"]
    channels = {k: (le, np.maximum.accumulate(curves[k]) - curves[k].min()) for k in "RGB"}
    # The axis' origin where green prints an 18 % grey, so the enlarger's solve starts on the curves' slope.
    origin = float(np.interp(-math.log10(0.18), channels["G"][1], le))
    le = le - origin
    channels = {k: (le, d) for k, (_, d) in channels.items()}
    inferred.append("the curves' log H origin: where green prints an 18 % grey; the enlarger's time absorbs it")
    base = channel @ np.array(lows)
    measured.append(f"characteristic curves, Status A, as printed ({curves_chart['confidence']} confidence), each less its D-min")
    inferred.append("densities held at the sheet's last point past the curves' printed range")
    inferred.append("the base's spectrum: the paper's own dyes in the amounts of the curves' least densities")
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
            "density_curves": [[round(float(v), 6) for v in row] for row in np.array([np.interp(le, *channels[k]) for k in "RGB"]).T],
        },
    }
    (OUT / f"{name}.json").write_text(json.dumps(profile, indent=1) + "\n")
    return name, inferred


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
            "developments": others,
        },
    }
    (OUT / f"{name}.json").write_text(json.dumps(profile, indent=1) + "\n")
    return name, inferred, len(others)


def status_a(dye, channel):
    centre, width = STATUS_A[channel]
    w = np.exp(-0.5 * ((WAVELENGTHS - centre) / (width / 2.3548)) ** 2)
    ok = ~np.isnan(dye)
    return -math.log10(float((10.0 ** -dye[ok] * w[ok]).sum() / w[ok].sum()))


def main():
    global STOCK
    dataset = Path(sys.argv[1])
    STOCK = {s["id"]: s for s in json.loads((dataset / "export/filmstocks.json").read_text())["stocks"]}
    gold = chart(STOCK[REFERENCE_SHEET], "characteristic_curves", "characteristic_curves")
    offset = gold["log_h_ref"] - speed_point(gold)
    basis = vision3_basis()
    OUT.mkdir(parents=True, exist_ok=True)
    names = []
    for stock_id, push in STOCKS:
        name, inferred = convert(stock_id, push, offset, basis)
        names.append(name)
        print(f"{name}: " + "; ".join(inferred[-2:-1]))
    for stock_id in PAPERS:
        name, inferred = convert_paper(stock_id)
        names.append(name)
        print(f"{name}: " + "; ".join(inferred))
    for stock_id, name, prefix, spectrum_of in BW_STOCKS:
        name, inferred, others = convert_bw(stock_id, name, prefix, spectrum_of)
        names.append(name)
        print(f"{name}: " + "; ".join(i for i in inferred if i.startswith(("the development", "the spectral", "the sensitivity's")))
              + f"; {others} other development{'s' if others != 1 else ''} kept")
    taken = {s for s, *_ in BW_STOCKS}
    for s in STOCK.values():
        if s["type"] == "bw_negative" and s["id"] not in taken:
            print(f"left out: {s['id']}: {BW_LEFT_OUT[s['id']]}")
    arms = "\n".join(f'    ("{n}", include_bytes!("../../resources/film/datasheet/{n}.json")),' for n in names)
    TABLE.write_text(
        "//! Every profile scripts/datasheet-profiles.py writes from the makers' data sheets, by its file's name; the\n"
        "//! script writes this table too.\n\n"
        f"pub const PROFILES: [(&str, &[u8]); {len(names)}] = [\n{arms}\n];\n"
    )


if __name__ == "__main__":
    main()
