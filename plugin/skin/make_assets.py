#!/usr/bin/env python3
"""Draws the PLAY page's artwork into this folder (a developer tool: the PNGs are committed, so a
plugin build does not need it). Needs Pillow (`pip install pillow`).

    python make_assets.py

- play_bg.png       1280 x 628   the page: charcoal ground, the head and lyrics panels, the grid,
                                 the highlight box of the syllable to sing next
- head_male.png, head_female.png, head_child.png
                    488 x 326 x 6 frames (rest, a, e, i, o, u): the head, its mouth on the vowel
                    being sung; the skin shows the strip of the body chosen by Voice
- knob_play.png / knob_play_bi.png   104 x 104 x 64 frames, the bottom row's knobs
- btn_*.png         buttons, each at the exact size the skin places it (images are not scaled)
- head_<body>_base.png, head_<body>_anim.png (1.3.3): the moving head. head_anim.json (written by
                    the engine fork's `--example anim_poses`) holds K poses per body, clustered from
                    the articulators of CHIMERA singing; every pose is drawn whole, then split into
                    a base (the rest pose with the moving region cleared) and a film strip of just
                    that region, which is the bounding box of every pixel that differs between
                    poses (so base + any frame = that pose, exactly). anim_layout.json: each body's
                    region, for the skin. `python make_assets.py --anim` makes only these.
- q<w>x<h>_<v>.png  a Quality button's film strip, 4 frames (one per Quality value: Eco, Std,
                    High, Lite): yellow on frame <v> (the value it selects), outlined otherwise
- clear<w>x<h>.png  a transparent image: the touch-taking Button drawn over a Quality button

The head is CHIMERA's own anatomy: head_outlines.json is written by the engine fork's
`cargo run --release -p chimera-mpc --example head_outlines` (HumanVocalTract, with the face and
cranium of the web app's profile.ts).
"""
import json
import math
import os

from PIL import Image, ImageChops, ImageDraw, ImageFilter

HERE = os.path.dirname(os.path.abspath(__file__))
SS = 3  # supersampling

BG = (16, 19, 21, 255)
PANEL = (9, 12, 13, 255)
CYAN = (70, 214, 230)
YELLOW = (240, 210, 60)
PINK = (236, 140, 178)
DIM = (58, 68, 72)

# layout shared with skins_post.py (tab coordinates)
HEAD_BOX = (16, 66, 492, 330)
LYR_BOX = (522, 66, 742, 330)
NOW_BOX = (780, 150, 240, 108)


def rgba(c, a=255):
    return (c[0], c[1], c[2], a)


def rrect(d, box, r, fill=None, outline=None, width=1):
    x, y, w, h = box
    d.rounded_rectangle([x * SS, y * SS, (x + w) * SS - 1, (y + h) * SS - 1], radius=r * SS, fill=fill,
                        outline=outline, width=width * SS)


def finish(img):
    return img.resize((img.width // SS, img.height // SS), Image.LANCZOS)


def mixc(c, a, base=PANEL):
    """`c` at alpha `a` over an opaque `base` (ImageDraw replaces pixels rather than blending)."""
    return tuple(round(base[k] + (c[k] - base[k]) * a / 255) for k in range(3)) + (255,)


def background():
    img = Image.new("RGBA", (1280 * SS, 628 * SS), BG)
    d = ImageDraw.Draw(img)
    for box in (HEAD_BOX, LYR_BOX):
        rrect(d, box, 10, fill=PANEL, outline=mixc(CYAN, 150, BG), width=2)
    # the head panel's grid
    x0, y0, w, h = HEAD_BOX
    for gx in range(x0 + 22, x0 + w - 4, 24):
        d.line([gx * SS, (y0 + 4) * SS, gx * SS, (y0 + h - 4) * SS], fill=mixc(CYAN, 22), width=SS)
    for gy in range(y0 + 18, y0 + h - 4, 24):
        d.line([(x0 + 4) * SS, gy * SS, (x0 + w - 4) * SS, gy * SS], fill=mixc(CYAN, 22), width=SS)
    # lyrics panel: the words row's underline and the lyric line's guide
    lx, ly, lw, lh = LYR_BOX
    d.line([(lx + 14) * SS, 134 * SS, (lx + lw - 14) * SS, 134 * SS], fill=mixc(CYAN, 60), width=SS)
    d.line([(lx + 14) * SS, 274 * SS, (lx + lw - 14) * SS, 274 * SS], fill=mixc(CYAN, 40), width=SS)
    # the syllable to sing next
    rrect(d, NOW_BOX, 10, fill=rgba(YELLOW))
    # hint strip and the knob row's top rule
    d.line([16 * SS, 434 * SS, 1264 * SS, 434 * SS], fill=mixc(CYAN, 90, BG), width=SS)
    for i in range(1, 8):
        x = 16 + i * 156
        d.line([x * SS, 448 * SS, x * SS, 614 * SS], fill=mixc(CYAN, 30, BG), width=SS)
    finish(img).save(os.path.join(HERE, "play_bg.png"), optimize=True)


def catmull(pts, n=6):
    if len(pts) < 3:
        return pts
    out = []
    p = [pts[0]] + pts + [pts[-1]]
    for i in range(1, len(p) - 2):
        p0, p1, p2, p3 = p[i - 1], p[i], p[i + 1], p[i + 2]
        for j in range(n):
            t = j / n
            t2, t3 = t * t, t * t * t
            out.append(tuple(0.5 * (2 * p1[k] + (-p0[k] + p2[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2
                                    + (-p0[k] + 3 * p1[k] - 3 * p2[k] + p3[k]) * t3) for k in range(2)))
    out.append(tuple(pts[-1]))
    return out


STYLE = {  # kind: (colour, alpha, width, glow)
    "skin": (CYAN, 255, 3.0, True),
    "lip": (CYAN, 255, 2.6, True),
    "wall": (CYAN, 235, 2.2, True),
    "velum": (CYAN, 235, 2.2, True),
    "epiglottis": (CYAN, 200, 1.8, False),
    "tongue": (PINK, 255, 2.4, True),
    "teeth": ((235, 245, 248), 210, 1.6, False),
    "jaw": (CYAN, 110, 1.6, False),
    "nasal": (CYAN, 120, 1.6, False),
    "eye": (CYAN, 150, 1.8, False),
    "spine": (CYAN, 70, 1.4, False),
    "brain": (CYAN, 0, 0, False),
}
HEAD_W, HEAD_H = HEAD_BOX[2] - 4, HEAD_BOX[3] - 4


def head_frame(pose, top):
    """One frame: body cm → pixels so that every body's head fills the panel alike (a smaller body
    is drawn a little smaller), the face to the right."""
    k = top / 22.9
    # from the neck to the brow (the crown is cut off, as on the MPC reference): the airway large
    y_lo, y_hi = -1.6 * k, 18.6 * k
    s = HEAD_H / (y_hi - y_lo) * (k ** 0.25)
    cx = HEAD_W * 0.62 - 9.25 * k * s * 0.55  # the nose tip (body x ≈ 9.25k) towards the right
    cy = HEAD_H + y_lo * s

    def px(p):
        return ((cx + p[0] * s) * SS, (cy - p[1] * s) * SS)

    sharp = Image.new("RGBA", (HEAD_W * SS, HEAD_H * SS), (0, 0, 0, 0))
    glow = Image.new("RGBA", sharp.size, (0, 0, 0, 0))
    ds, dg = ImageDraw.Draw(sharp), ImageDraw.Draw(glow)
    for ln in pose["lines"]:
        col, a, w, gl = STYLE[ln["kind"]]
        if a == 0:
            continue
        pts = [tuple(p) for p in ln["pts"]]
        if ln["smooth"]:
            pts = catmull(pts)
        if ln["closed"]:
            pts = pts + [pts[0]]
        xy = [px(p) for p in pts]
        if ln["kind"] == "tongue":
            ds.polygon(xy, fill=rgba(PINK, 46))
        ds.line(xy, fill=rgba(col, a), width=max(1, round(w * SS)), joint="curve")
        if gl:
            dg.line(xy, fill=rgba(col, 150), width=round(w * SS * 3), joint="curve")
    glow = glow.filter(ImageFilter.GaussianBlur(5 * SS))
    out = Image.alpha_composite(glow, sharp)
    return finish(out)


def heads():
    data = json.load(open(os.path.join(HERE, "head_outlines.json"), encoding="utf-8"))
    strips = []
    for b in data["bodies"]:
        top = max(p[1] for ln in b["poses"][0]["lines"] if ln["kind"] == "skin" for p in ln["pts"])
        strips.append(("head_%s.png" % b["name"], [head_frame(pose, top) for pose in b["poses"]]))
    for name, frames in strips:
        strip = Image.new("RGBA", (HEAD_W, HEAD_H * len(frames)), (0, 0, 0, 0))
        for i, f in enumerate(frames):
            strip.paste(f, (0, i * HEAD_H))
        strip.save(os.path.join(HERE, name), optimize=True)
    return {b["name"]: {p["name"]: p["length"] for p in b["poses"]} for b in data["bodies"]}


def knob(bipolar, n=64, size=104):
    strip = Image.new("RGBA", (size, size * n), (0, 0, 0, 0))
    a0, a1 = 135.0, 405.0  # PIL degrees: clockwise from 3 o'clock; 270° sweep from bottom-left
    for i in range(n):
        t = i / (n - 1)
        img = Image.new("RGBA", (size * SS, size * SS), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        c = size * SS / 2
        r = size * SS / 2 - 6 * SS
        box = [c - r, c - r, c + r, c + r]
        d.arc(box, a0, a1, fill=rgba(DIM), width=7 * SS)
        av = a0 + t * (a1 - a0)
        if bipolar:
            mid = (a0 + a1) / 2
            lo, hi = sorted((mid, av))
            if hi - lo > 0.5:
                d.arc(box, lo, hi, fill=rgba(CYAN), width=7 * SS)
        elif t > 0.004:
            d.arc(box, a0, av, fill=rgba(CYAN), width=7 * SS)
        ri = r - 13 * SS
        d.ellipse([c - ri, c - ri, c + ri, c + ri], fill=(26, 31, 34, 255), outline=(48, 58, 62, 255), width=2 * SS)
        rad = math.radians(av)
        d.line([c + math.cos(rad) * ri * 0.25, c + math.sin(rad) * ri * 0.25, c + math.cos(rad) * ri * 0.86,
                c + math.sin(rad) * ri * 0.86], fill=(255, 255, 255, 255), width=4 * SS)
        strip.paste(finish(img), (0, i * size))
    strip.save(os.path.join(HERE, "knob_play_bi.png" if bipolar else "knob_play.png"), optimize=True)


BUTTONS = {
    # name: (w, h, style)
    "btn_mode_on": (128, 46, "yellow"),
    "btn_mode_off": (128, 46, "dim"),
    "btn_mode_wide_off": (150, 46, "dim"),
    "btn_restart": (120, 46, "outline"),
    "btn_type": (132, 46, "cyan"),
    "btn_live": (240, 36, "outline"),
}


def knob_off(size=104):
    """The placeholder slot's knob: unlit, no pointer."""
    img = Image.new("RGBA", (size * SS, size * SS), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    c = size * SS / 2
    r = size * SS / 2 - 6 * SS
    d.arc([c - r, c - r, c + r, c + r], 135, 405, fill=(36, 42, 45, 255), width=7 * SS)
    ri = r - 13 * SS
    d.ellipse([c - ri, c - ri, c + ri, c + ri], fill=(20, 24, 26, 255), outline=(38, 45, 48, 255), width=2 * SS)
    finish(img).save(os.path.join(HERE, "knob_off.png"), optimize=True)


def buttons():
    for name, (w, h, style) in BUTTONS.items():
        img = Image.new("RGBA", (w * SS, h * SS), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        box = (1, 1, w - 2, h - 2)
        if style == "yellow":
            rrect(d, box, 8, fill=rgba(YELLOW))
        elif style == "cyan":
            rrect(d, box, 8, fill=rgba(CYAN))
        elif style == "outline":
            rrect(d, box, 8, fill=(14, 20, 22, 255), outline=rgba(CYAN, 220), width=2)
        else:
            rrect(d, box, 8, fill=(14, 17, 18, 255), outline=rgba(DIM), width=2)
        finish(img).save(os.path.join(HERE, name + ".png"), optimize=True)


def anim_heads():
    data = json.load(open(os.path.join(HERE, "head_anim.json"), encoding="utf-8"))
    layout = {"k": data["k"], "bodies": {}}
    decoded = 0
    for b in data["bodies"]:
        poses = b["poses"]
        top = max(p[1] for ln in poses[0]["lines"] if ln["kind"] == "skin" for p in ln["pts"])
        frames = [head_frame(pose, top) for pose in poses]
        # the moving region: where any pose differs from the rest pose (alpha or colour > 2/255)
        box = None
        for f in frames[1:]:
            r_, g_, b_, a_ = ImageChops.difference(frames[0], f).split()
            diff = ImageChops.lighter(ImageChops.lighter(r_, g_), ImageChops.lighter(b_, a_)).point(lambda v: 255 if v > 2 else 0)
            bb = diff.getbbox()
            if bb:
                box = bb if box is None else (min(box[0], bb[0]), min(box[1], bb[1]), max(box[2], bb[2]), max(box[3], bb[3]))
        x0, y0, x1, y1 = box
        x0, y0, x1, y1 = max(0, x0 - 1), max(0, y0 - 1), min(HEAD_W, x1 + 1), min(HEAD_H, y1 + 1)
        w, h = x1 - x0, y1 - y0
        # outside the region every pose is the rest pose (glow tails differ by at most 2/255): so
        # base + frame = pose, with no visible seam
        seam = 0
        for f in frames[1:]:
            a, r = frames[0].copy(), f.copy()
            for im in (a, r):
                im.paste((0, 0, 0, 0), (x0, y0, x1, y1))
            worst = max(hi for lo, hi in ImageChops.difference(a, r).getextrema())
            assert worst <= 2, "a pose differs outside the moving region by %d/255" % worst
            seam = max(seam, worst)
        base = frames[0].copy()
        base.paste((0, 0, 0, 0), (x0, y0, x1, y1))
        base.save(os.path.join(HERE, "head_%s_base.png" % b["name"]), optimize=True)
        strip = Image.new("RGBA", (w, h * len(frames)), (0, 0, 0, 0))
        for i, f in enumerate(frames):
            strip.paste(f.crop((x0, y0, x1, y1)), (0, i * h))
        strip.save(os.path.join(HERE, "head_%s_anim.png" % b["name"]), optimize=True)
        layout["bodies"][b["name"]] = [x0, y0, w, h]
        decoded += (HEAD_W * HEAD_H + w * h * len(frames)) * 4
        print("  %s: moving region %d x %d at (%d, %d), %d frames, largest difference outside it %d/255" % (b["name"], w, h, x0, y0, len(frames), seam))
    json.dump(layout, open(os.path.join(HERE, "anim_layout.json"), "w"), indent=1)
    print("  decoded size of the head images: %.1f MB (1.3.2's six-frame heads: %.1f MB)"
          % (decoded / 1e6, 3 * 6 * HEAD_W * HEAD_H * 4 / 1e6))


def quality_strips():
    for w, h in ((74, 42), (62, 40)):
        Image.new("RGBA", (w, h), (0, 0, 0, 0)).save(os.path.join(HERE, "clear%dx%d.png" % (w, h)), optimize=True)
        for v in range(4):
            strip = Image.new("RGBA", (w, h * 4), (0, 0, 0, 0))
            for k in range(4):
                img = Image.new("RGBA", (w * SS, h * SS), (0, 0, 0, 0))
                d = ImageDraw.Draw(img)
                box = (1, 1, w - 2, h - 2)
                if k == v:
                    rrect(d, box, 8, fill=rgba(YELLOW))
                else:
                    rrect(d, box, 8, fill=(14, 20, 22, 255), outline=rgba(CYAN, 220), width=2)
                strip.paste(finish(img), (0, k * h))
            strip.save(os.path.join(HERE, "q%dx%d_%d.png" % (w, h, v)), optimize=True)


if __name__ == "__main__":
    if "--anim" in __import__("sys").argv:
        anim_heads()
        raise SystemExit
    if "--buttons" in __import__("sys").argv:
        # only the page, the buttons and strips (1.3.1+): the head and the knobs stay as they are
        background()
        buttons()
        quality_strips()
        raise SystemExit
    quality_strips()
    background()
    lengths = heads()
    knob(False)
    knob(True)
    knob_off()
    buttons()
    json.dump(lengths, open(os.path.join(HERE, "tract_lengths.json"), "w"), indent=1)
    print("ok", lengths)
