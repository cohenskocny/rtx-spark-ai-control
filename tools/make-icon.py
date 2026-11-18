"""Generate the RTX Spark application icon and its Windows asset set.

The mark is drawn here rather than shipped as stock art: a four-point spark cut
from refractive cobalt glass with an electric-blue inner glow. Everything is
rendered at 4x and reduced with LANCZOS, so the small sizes keep a clean
silhouette instead of a blurry star.

    python tools/make-icon.py                 # candidates + contact sheet
    python tools/make-icon.py --export spark-facets   # write src-tauri/icons/*

Candidates are written to assets/icon-candidates/ for comparison; the export
step writes the PNG/ICO set Tauri builds into the executable.
"""
from __future__ import annotations

import argparse
import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
CANDIDATES = ROOT / "assets" / "icon-candidates"
ICONS = ROOT / "src-tauri" / "icons"

RENDER = 1024          # working resolution, 4x the largest shipped size
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

# Cobalt ramp, brightest core to dark edge. Kept identical to the palette in
# src/styles/base.css so the icon and the interface read as one system.
CORE = (226, 238, 255)
MID = (122, 167, 255)
DEEP = (47, 88, 214)
EDGE = (14, 28, 88)
ELECTRIC = (150, 205, 255)


def spark_points(size: int, waist: float = 0.205, inset: float = 0.965):
    """Four-point spark: arms on N/E/S/W, concave waist between them."""
    cx = cy = size / 2.0
    reach = (size / 2.0) * inset
    k = reach * waist
    return [
        (cx, cy - reach),          # N tip
        (cx + k, cy - k),
        (cx + reach, cy),          # E tip
        (cx + k, cy + k),
        (cx, cy + reach),          # S tip
        (cx - k, cy + k),
        (cx - reach, cy),          # W tip
        (cx - k, cy - k),
    ]


def radial_gradient(size: int, stops) -> Image.Image:
    """Concentric-ellipse radial gradient; cheap and smooth after reduction."""
    img = Image.new("RGB", (size, size), EDGE)
    draw = ImageDraw.Draw(img)
    cx = cy = size / 2.0
    steps = 256
    for i in range(steps, 0, -1):
        t = i / steps
        colour = _sample(stops, t)
        radius = t * size * 0.72
        draw.ellipse(
            [cx - radius, cy - radius * 0.98, cx + radius, cy + radius * 0.98],
            fill=colour,
        )
    return img


def _sample(stops, t: float):
    """Piecewise-linear colour lookup over [(position, rgb), ...]."""
    for (p0, c0), (p1, c1) in zip(stops, stops[1:]):
        if p0 <= t <= p1:
            span = max(1e-6, p1 - p0)
            f = (t - p0) / span
            return tuple(round(a + (b - a) * f) for a, b in zip(c0, c1))
    return stops[-1][1]


def spark_mask(size: int, waist: float, blur: float = 0.0) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).polygon(spark_points(size, waist), fill=255)
    if blur:
        mask = mask.filter(ImageFilter.GaussianBlur(blur))
    return mask


def build_spark(size: int, facets: bool, glow: bool = True,
                bright: bool = False) -> Image.Image:
    """The spark itself on transparency.

    `bright` lifts the ramp when the spark sits on a cobalt tile: blue-on-blue
    loses the silhouette at 24-32 px, while a near-white core keeps it while
    the arms still read as cobalt glass.
    """
    stops = [
        (0.00, (248, 251, 255) if bright else CORE),
        (0.26, (196, 222, 255) if bright else MID),
        (0.60, (128, 170, 255) if bright else DEEP),
        (0.88, (58, 96, 224) if bright else EDGE),
        (1.00, (40, 72, 190) if bright else EDGE),
    ]
    base = radial_gradient(size, stops).convert("RGBA")
    mask = spark_mask(size, waist=0.205)
    base.putalpha(mask)

    if facets:
        # Each arm is a separate facet: alternating brightness reads as a cut
        # gem under a single light, without any outline detail that would turn
        # to mud at 16 px.
        cx = cy = size / 2.0
        pts = spark_points(size, waist=0.205)
        shade = [1.10, 0.88, 1.0, 0.93, 1.06, 0.9, 1.02, 0.94]
        overlay = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        draw = ImageDraw.Draw(overlay)
        for i, point in enumerate(pts):
            nxt = pts[(i + 1) % len(pts)]
            f = shade[i]
            if abs(f - 1.0) < 0.01:
                continue
            if f > 1.0:
                colour = (255, 255, 255, round((f - 1.0) * 150))
            else:
                colour = (6, 14, 46, round((1.0 - f) * 190))
            draw.polygon([(cx, cy), point, nxt], fill=colour)
        overlay.putalpha(Image.composite(overlay.getchannel("A"), Image.new("L", (size, size), 0), mask))
        base = Image.alpha_composite(base, overlay)

    if glow:
        # Inner glow: a shrunk, blurred copy of the silhouette in electric blue,
        # clipped to the shape, plus a soft outer bloom.
        inner = spark_mask(size, waist=0.205)
        shrunk = Image.new("L", (size, size), 0)
        ImageDraw.Draw(shrunk).polygon(
            spark_points(size, waist=0.30, inset=0.58), fill=120
        )
        shrunk = shrunk.filter(ImageFilter.GaussianBlur(size * 0.045))
        shrunk = Image.composite(shrunk, Image.new("L", (size, size), 0), inner)
        glow_layer = Image.new("RGBA", (size, size), ELECTRIC + (0,))
        glow_layer.putalpha(shrunk)
        base = Image.alpha_composite(base, glow_layer)

        bloom = spark_mask(size, waist=0.205, blur=size * 0.028)
        bloom = bloom.point(lambda v: int(v * (0.18 if bright else 0.32)))
        bloom_layer = Image.new("RGBA", (size, size), ELECTRIC + (0,))
        bloom_layer.putalpha(bloom)
        base = Image.alpha_composite(bloom_layer, base)

    # Lit edge: the surface catches light along the top-left, which is what
    # makes the shape read as glass rather than a flat silhouette.
    rim = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(rim)
    width = max(2, int(size * 0.010))
    draw.line(
        spark_points(size, waist=0.205)[3:] + spark_points(size, waist=0.205)[:5],
        fill=(196, 224, 255, 96),
        width=width,
        joint="curve",
    )
    rim.putalpha(Image.composite(rim.getchannel("A"), Image.new("L", (size, size), 0), mask))
    base = Image.alpha_composite(base, rim)
    return base


def variant(name: str) -> Image.Image:
    size = RENDER
    if name == "spark-facets":
        return build_spark(size, facets=True)
    if name == "spark-solid":
        return build_spark(size, facets=False)
    if name == "spark-glass":
        # A cobalt glass tile with the faceted spark set into it: the tile
        # carries the silhouette at 16 px, the spark carries the identity from
        # 32 px up, and the lit upper edge keeps the liquid-glass reading.
        canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        tile = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        d = ImageDraw.Draw(tile)
        pad = int(size * 0.035)
        box = [pad, pad, size - pad, size - pad]
        radius = int(size * 0.235)
        # Vertical cobalt gradient, lighter at the top like the app's glass.
        for i in range(size):
            t = i / (size - 1)
            colour = _sample(
                [(0.0, (58, 96, 226)), (0.55, (26, 48, 148)), (1.0, (9, 17, 54))],
                t,
            )
            d.line([(0, i), (size, i)], fill=colour + (255,))
        grad_mask = Image.new("L", (size, size), 0)
        ImageDraw.Draw(grad_mask).rounded_rectangle(box, radius=radius, fill=255)
        tile.putalpha(grad_mask)
        canvas = Image.alpha_composite(canvas, tile)

        # Lit upper edge, clipped to the tile: a sheen that fades downward, plus
        # a 1-px-class outline. No arcs across the face — that read as a stray
        # line rather than a glass highlight.
        edges = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        ed = ImageDraw.Draw(edges)
        sheen_h = int(size * 0.26)
        for i in range(sheen_h):
            a = int(46 * (1 - i / sheen_h) ** 1.5)
            ed.line([(0, i), (size, i)], fill=(214, 232, 255, a))
        ed.rounded_rectangle(
            box, radius=radius, outline=(150, 190, 255, 62), width=max(2, int(size * 0.006))
        )
        edges.putalpha(
            Image.composite(
                edges.getchannel("A"), Image.new("L", (size, size), 0), grad_mask
            )
        )
        canvas = Image.alpha_composite(canvas, edges)

        mark = build_spark(size, facets=True, bright=True)
        shrink = 0.80
        mark = mark.resize((int(size * shrink), int(size * shrink)), Image.LANCZOS)
        off = (size - mark.width) // 2
        canvas.alpha_composite(mark, (off, off))
        return canvas
    if name == "spark-bloom":
        return build_spark(size, facets=True).filter(ImageFilter.GaussianBlur(0))
    if name == "spark-disc":
        # Glass disc behind the spark: keeps the mark legible on a light taskbar
        # and matches the liquid-glass surfaces inside the app.
        canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        disc = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        d = ImageDraw.Draw(disc)
        pad = int(size * 0.02)
        d.ellipse([pad, pad, size - pad, size - pad], fill=(10, 20, 56, 232))
        d.ellipse(
            [pad, pad, size - pad, size - pad],
            outline=(120, 168, 255, 72),
            width=max(2, int(size * 0.012)),
        )
        canvas = Image.alpha_composite(canvas, disc)
        mark = build_spark(size, facets=True)
        shrink = 0.74
        mark = mark.resize((int(size * shrink), int(size * shrink)), Image.LANCZOS)
        off = (size - mark.width) // 2
        canvas.alpha_composite(mark, (off, off))
        return canvas
    if name == "spark-tile":
        # Rounded cobalt tile with the spark knocked out: the most legible
        # silhouette at 16 px, but it loses the glass refraction.
        canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        tile = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        d = ImageDraw.Draw(tile)
        pad = int(size * 0.04)
        d.rounded_rectangle(
            [pad, pad, size - pad, size - pad],
            radius=int(size * 0.22),
            fill=(24, 46, 140, 255),
        )
        canvas = Image.alpha_composite(canvas, tile)
        cut = Image.new("L", (size, size), 0)
        ImageDraw.Draw(cut).polygon(
            spark_points(size, waist=0.24, inset=0.66), fill=255
        )
        knock = Image.new("RGBA", (size, size), (210, 228, 255, 255))
        knock.putalpha(cut)
        canvas = Image.alpha_composite(canvas, knock)
        return canvas
    raise SystemExit(f"unknown variant {name}")


def small(img: Image.Image, px: int) -> Image.Image:
    return img.resize((px, px), Image.LANCZOS)


def contact_sheet(names: list[str]) -> Path:
    """Every candidate at the sizes the taskbar and title bar actually use.

    Each variant gets two rows: the dark cobalt field the app paints, and a
    light surface, because a transparent icon has to survive both a dark
    taskbar and a light Explorer window.
    """
    sizes = [128, 64, 48, 32, 24, 16]
    cell = 150
    pad = 16
    per_variant = 2
    rows = len(names) * per_variant
    width = pad + sum(cell for _ in sizes) + pad
    height = pad + rows * (cell + 22) + pad
    sheet = Image.new("RGB", (width, height), (8, 12, 28))
    draw = ImageDraw.Draw(sheet)

    for r, name in enumerate(names):
        img = variant(name)
        for b, bg in enumerate([(8, 12, 28), (236, 240, 248)]):
            top = pad + (r * per_variant + b) * (cell + 22)
            label = f"{name}  ({'dark' if b == 0 else 'light'})"
            draw.text((pad, top), label, fill=(206, 219, 255) if b == 0 else (30, 40, 70))
            x = pad
            for px in sizes:
                tile = small(img, px)
                cellimg = Image.new("RGB", (cell, cell), bg)
                cellimg.paste(tile, ((cell - px) // 2, (cell - px) // 2), tile)
                sheet.paste(cellimg, (x, top + 20))
                x += cell
    CANDIDATES.mkdir(parents=True, exist_ok=True)
    out = CANDIDATES / "contact-sheet.png"
    sheet.save(out)
    return out


def export(name: str) -> None:
    """Write the Windows asset set from the chosen candidate."""
    img = variant(name)
    ICONS.mkdir(parents=True, exist_ok=True)
    small(img, 32).save(ICONS / "32x32.png")
    small(img, 128).save(ICONS / "128x128.png")
    small(img, 256).save(ICONS / "128x128@2x.png")
    small(img, 512).save(ICONS / "icon.png")
    small(img, 256).save(ICONS / "Square310x310Logo.png")
    # Multi-resolution ICO: Windows picks the right frame per context, so the
    # executable, title bar and taskbar each get a size drawn for them.
    frames = [small(img, px) for px in ICO_SIZES]
    frames[-1].save(
        ICONS / "icon.ico",
        format="ICO",
        sizes=[(px, px) for px in ICO_SIZES],
        append_images=frames[:-1],
    )
    # Source-scale PNG for documentation and the contact sheet.
    (CANDIDATES).mkdir(parents=True, exist_ok=True)
    small(img, 256).save(CANDIDATES / f"{name}-256.png")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--export", default="", help="variant to write into src-tauri/icons")
    args = ap.parse_args()
    if args.export:
        export(args.export)
        print(f"exported {args.export} to {ICONS}")
        return 0
    names = ["spark-glass", "spark-facets", "spark-solid", "spark-disc", "spark-tile"]
    for name in names:
        img = variant(name)
        CANDIDATES.mkdir(parents=True, exist_ok=True)
        small(img, 256).save(CANDIDATES / f"{name}-256.png")
        small(img, 16).save(CANDIDATES / f"{name}-16.png")
    print(contact_sheet(names))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
