"""An approximate PC rendering of the PLAY page from a built Plugin Skins folder (for docs only).

    python scripts/skin_preview.py "<build>/Gator Audio - VST - Chimera Voice/Plugin Skins" out.png [kb]

It draws the skin's real images and layout, but text with substitute fonts (Segoe UI, Windows) and
fixed sample values (the plugin's display text for a new instance: the default words, Lite,
Animation and Live Lyrics on). It is not an MPC screenshot; the MPC's own renderer and fonts
differ. "kb" draws the keyboard panel open. Needs Pillow.
"""
import json, os, re, sys
from PIL import Image, ImageDraw, ImageFont

skins, out = sys.argv[1], sys.argv[2]
kb_open = len(sys.argv) > 3 and sys.argv[3] == "kb"
tui = json.load(open(os.path.join(skins, "TUI.json"), encoding="utf-8"))["pageData"]
defs = {d["key"]: d["value"] for d in tui["componentDefinitions"]["localComponentDefinitions"]}
# sample state: plain step indices / normalised
val = {86: 0, 1: 0, 18: 3, 75: 0, 76: 1 if kb_open else 0, 15: 0, 5: 0.26, 6: 0.15, 11: 0.1, 7: 0.29, 2: 0.5, 0: 0.9}
disp = {1: "Male", 5: "7.2 cmH2O", 6: "0.15", 11: "0.10", 7: "35 ct", 2: "+0 st", 0: "0.0 dB", 77: "", 20: "Hel-", 78: " lo there I am",
        79: "NEXT 1/10", 84: "a voice made of air", 85: "LIVE LYRICS: ON", 87: "ANIMATION: ON", 74: "(sung) 38 chars", 81: "Hello there, I am a voice made", 82: "of air.|", 83: "", 19: "0% pk0", 18: "Lite"}
PNAMES = [p["name"] for p in json.load(open(os.path.join(skins, "..", "..", "params.json"), encoding="utf-8"))["params"]]
steps = {r["key"]: len(r["value"]) for r in tui["replacementStrings"]}
FONT = {"Bold": "segoeuib.ttf", "SemiBold": "segoeuib.ttf", "Regular": "segoeui.ttf"}

def pnum(h):
    m = re.search(r"Parameter (\d+)$", h)
    return int(m.group(1)) if m else None

def valid(h):
    if h.startswith("NonZero/"):
        return val.get(pnum(h), 0) != 0
    m = re.match(r"IndexedEnabling/(\d+)/(\d+)/Parameter (\d+)", h)
    if m:
        return int(val.get(int(m.group(3)), 0)) == int(m.group(1))
    return True

def text_of(h, kind, data):
    if kind == "Text":
        return data["text"]
    pre = ""
    m = re.match(r"WithPrefix\[(.*?)\]/(.*)", h)
    if m:
        pre, h = m.group(1), m.group(2)
    if kind == "Name":
        return pre + PNAMES[pnum(h)]
    return pre + disp.get(pnum(h), "?")

def draw(img, comp_items, ox, oy, remap):
    d = ImageDraw.Draw(img)
    for e in comp_items:
        b = e["bounds"]
        if not all(valid(h) for h in b.get("additionalInvalidatingHandles", [])):
            continue
        if b["whenVisible"] == "WhenFocussed":
            continue
        rm = dict(remap)
        for m in e["handle remapping"]["map"]:
            rm[m["key"]] = rm.get(m["value"], m["value"])
        x, y, w, h = [float(v) for v in b["bounds"].split()]
        if b["boundsType"] == "Proportional":
            x, y, w, h = 0, 0, remap["__w"], remap["__h"]
        x += ox; y += oy
        cd = e["componentData"]; t = cd["type"]; data = cd["data"]
        res = lambda hh: rm.get(hh, hh)
        if t == "Image":
            im = Image.open(os.path.join(skins, data["image"])).convert("RGBA")
            if data.get("imageType") == "FilmStrip":
                n = data["numFrames"]; fh = im.height // n
                k = int(val.get(pnum(res(data["handleName"])), 0)); im = im.crop((0, k * fh, im.width, (k + 1) * fh))
            im = im.resize((int(w), int(h))); img.alpha_composite(im, (int(x), int(y)))
        elif t == "Button":
            hh = res(data["handleName"]); on = int(val.get(pnum(hh), 0)) == data["buttonId"]
            im = Image.open(os.path.join(skins, data["onImage" if on else "offImage"])).convert("RGBA")
            img.alpha_composite(im, (int(x + (w - im.width) / 2), int(y + (h - im.height) / 2)))
        elif t == "Knob" and data["knobType"] == "FilmStrip":
            im = Image.open(os.path.join(skins, data["filmStrip"])).convert("RGBA")
            n = data["numFrames"]; fh = im.height // n
            v = val.get(pnum(res("Data")), 0.5); k = int(round(v * (n - 1)))
            img.alpha_composite(im.crop((0, k * fh, im.width, (k + 1) * fh)), (int(x), int(y)))
        elif t == "Label":
            ts = data["textStyle"]; f = ts["font"]
            font = ImageFont.truetype(FONT.get(f["style"], "segoeui.ttf"), int(f["height"] * 0.9))
            s = text_of(res(data["handleName"]), data["type"], data)
            if ts["case"] == "Upper Case": s = s.upper()
            c = ts["colour"]; col = (int(c[2:4], 16), int(c[4:6], 16), int(c[6:8], 16), int(c[0:2], 16))
            tw = d.textlength(s, font=font)
            j = ts["justification"]
            tx = x if "left" in j else (x + w - tw if "right" in j else x + (w - tw) / 2)
            ty = y + (h - f["height"]) / 2
            dd = ImageDraw.Draw(img); dd.text((tx, ty), s, font=font, fill=col)
        elif t in defs:
            cdef = defs[t]
            bg = cdef["backgroundData"]["unfocussed"]
            if bg.get("image"):
                im = Image.open(os.path.join(skins, bg["image"])).convert("RGBA").resize((int(w), int(h)))
                img.alpha_composite(im, (int(x), int(y)))
            elif bg["colour"] not in ("0", "00000000", ""):
                c = bg["colour"]; ImageDraw.Draw(img).rectangle([x, y, x + w - 1, y + h - 1], fill=(int(c[2:4], 16), int(c[4:6], 16), int(c[6:8], 16)))
            r2 = {"Data": rm.get("Data", "Data"), "__w": w, "__h": h}
            draw(img, cdef["componentsData"], x, y, r2)

tab = tui["tabs"][0]
img = Image.new("RGBA", (1280, 628), (0, 0, 0, 255))
root = tab["componentDefinition"]
bgimg = root["backgroundData"]["unfocussed"]["image"]
img.alpha_composite(Image.open(os.path.join(skins, bgimg)).convert("RGBA"))
draw(img, root["componentsData"], 0, 0, {"__w": 1280, "__h": 628})
img.save(out)
print([t["tabName"] for t in tui["tabs"]])
