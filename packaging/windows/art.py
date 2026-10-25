"""The installer's pictures drawn as the splash (ds/splash.slint), the caption raised over the button row.
`python art.py VERSION OUT_DIR`; logo.png is oni-logo.svg rasterised at 4x."""
import os
import sys

from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
UI = os.path.join(HERE, "..", "..", "crates", "oni-ui")
SPLASH = os.path.join(UI, "ui", "splash")
FONT = os.path.join(UI, "fonts", "Inter.ttf")
ICON = os.path.join(UI, "ui", "icons", "app-icon.png")

# Inno Setup's background area at 100, 150, 200 and 250 % (497:360 kept), and the small image's.
BACKS = [(596, 432), (894, 648), (1272, 922), (1630, 1148)]
SMALLS = [55, 83, 110, 138]
# `SplashStyle` on its 720 x 480 window: logo 246 x 64, insets 28 / 32 / 28, 12 between lines.
SPLASH_HEIGHT = 480
LOGO = (246, 64)
INSET_LEADING, INSET_TRAILING, INSET_BOTTOM, LINES, INDENT, CREDIT_GAP = 28, 32, 28, 12, 4, 2
TEXT_SIZE = 16
# The wizard's button row covers the window's foot: the caption sits above it, a shade under both.
BUTTONS = 0.12
FOOT_SHADE, HEAD_SHADE = 0.78, 0.55
# Pages other than the welcome and the finish keep the photograph dimmed to this, for their text.
DIM = 0.28


def picture(version):
    """The version's own picture, else the newest, as `splash::picture`."""
    names = sorted(f[:-4] for f in os.listdir(SPLASH) if f.endswith(".jpg"))
    name = version if version in names else max(names, key=lambda v: tuple(int(p) for p in v.split(".")))
    credit = open(os.path.join(SPLASH, name + ".txt"), encoding="utf-8").read().strip()
    return Image.open(os.path.join(SPLASH, name + ".jpg")).convert("RGB"), credit


def cover(photo, size):
    w, h = size
    s = max(w / photo.width, h / photo.height)
    scaled = photo.resize((round(photo.width * s), round(photo.height * s)), Image.LANCZOS)
    x, y = (scaled.width - w) // 2, (scaled.height - h) // 2
    return scaled.crop((x, y, x + w, y + h))


def shade(image, start, end, alpha_start, alpha_end):
    """Black from `alpha_start` to `alpha_end` between two rows, a straight ramp."""
    over = Image.new("RGBA", image.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(over)
    for y in range(int(start), int(end)):
        t = (y - start) / max(1, end - start)
        draw.line([(0, y), (image.width, y)], fill=(0, 0, 0, round(255 * (alpha_start + (alpha_end - alpha_start) * t))))
    return Image.alpha_composite(image, over)


def font(size, weight):
    f = ImageFont.truetype(FONT, size)
    f.set_variation_by_axes([14, weight])
    return f


def back(photo, credit, version, size):
    w, h = size
    s = h / SPLASH_HEIGHT
    image = cover(photo, size).convert("RGBA")
    image = shade(image, 0, h * 0.42, HEAD_SHADE, 0)
    image = shade(image, h * 0.5, h, 0, FOOT_SHADE)
    foot = h * (1 - BUTTONS)
    draw = ImageDraw.Draw(image)
    line, caption = font(round(TEXT_SIZE * s), 400), font(round(TEXT_SIZE * s), 300)
    text = f"V{version}"
    _, top, _, bottom = draw.textbbox((0, 0), text, font=line)
    text_y = foot - INSET_BOTTOM * s - bottom
    draw.text(((INSET_LEADING + INDENT) * s, text_y), text, font=line, fill="white")
    logo = Image.open(os.path.join(HERE, "logo.png")).convert("RGBA")
    logo = logo.resize((round(LOGO[0] * s), round(LOGO[1] * s)), Image.LANCZOS)
    image.alpha_composite(logo, (round(INSET_LEADING * s), round(text_y + top - LINES * s - logo.height)))
    right = w - INSET_TRAILING * s
    credit_y = foot - INSET_BOTTOM * s - draw.textbbox((0, 0), credit, font=line)[3]
    draw.text((right, credit_y), credit, font=line, fill="white", anchor="ra")
    photo_box = draw.textbbox((0, 0), "Photo", font=caption)
    draw.text((right, credit_y + top - CREDIT_GAP * s - photo_box[3]), "Photo", font=caption, fill="white", anchor="ra")
    return image.convert("RGB")


def page(photo, size):
    dim = cover(photo, size)
    return Image.blend(Image.new("RGB", size, "black"), dim, DIM)


def main():
    version, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    photo, credit = picture(version)
    for size in BACKS:
        back(photo, credit, version, size).save(os.path.join(out, f"splash-{size[0]}.png"))
        page(photo, size).save(os.path.join(out, f"page-{size[0]}.png"))
    icon = Image.open(ICON).convert("RGBA")
    for side in SMALLS:
        icon.resize((side, side), Image.LANCZOS).save(os.path.join(out, f"small-{side}.png"))


if __name__ == "__main__":
    main()
