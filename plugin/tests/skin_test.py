#!/usr/bin/env python3
"""Regression test of the generated skin (a built Plugin Skins folder, or a packaged zip), against
what the original MPC One did with earlier versions.

    python skin_test.py <Plugin Skins dir | package .zip> [params.json]

Visibility is evaluated the way the hardware behaved: an entry with showWhenDataModelInvalid
"Show" is ALWAYS drawn (1.3.0-rc1 drew the keyboard over PLAY that way), and a "Hide" entry is
drawn only while its own handle and its additionalInvalidatingHandles are valid.

Checks:
- tabs are PLAY, VOICE (1.3.1: no LYRICS tab; 1.3.2: no KEYS tab); with Keyboard off no tab shows
  a key, with it on PLAY shows the panel
- the keyboard panel hides the MPC's Q-Link highlight (1.3.1-rc1: the knob row's highlight was drawn
  over SHIFT and SPACE): hideQLinkBounds, component version 4, keys in the documented order
- PLAY (1.3.2): no Start Syllable readout or help text; the lyric line, the next line, Live Lyrics
  and CPU are there
- no Text label is bound to "Invalid" (1.3.0-rc2: such captions were blank on DONE and BACK)
- no entry drawn above a Button overlaps it (1.3.0-rc2: Labels over PLAY's Quality buttons took
  their taps); the four Quality buttons on PLAY and VOICE read LITE ECO STD HIGH and set 3 0 1 2
- the keyboard: DONE and CANCEL captioned, CANCEL and DONE close the panel, no Load File key,
  Backspace far from Clear All, a wide Space, the status and three draft lines bound
- no MY WORDS / FILE switch or lyric-set selector on PLAY
- PLAY's Q-Links 13-16 are Body, Breath, Breathy, Rough; every Q-Link names a real parameter
- every image the skin names is present
"""
import json
import os
import re
import sys
import zipfile


def load(path):
    if path.endswith(".zip"):
        z = zipfile.ZipFile(path)
        names = z.namelist()
        tui = next(n for n in names if n.endswith("Plugin Skins/TUI.json"))
        base = tui.rsplit("/", 1)[0] + "/"
        ql = json.loads(z.read(base + "Q-Links.json"))
        return json.loads(z.read(tui)), ql, {n[len(base):] for n in names if n.startswith(base)}
    tui = json.load(open(os.path.join(path, "TUI.json"), encoding="utf-8"))
    ql = json.load(open(os.path.join(path, "Q-Links.json"), encoding="utf-8"))
    return tui, ql, set(os.listdir(path))


def rect(e):
    return [float(v) for v in e["bounds"]["bounds"].split()]


def overlap(a, b):
    return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]


def main():
    tui, ql, files = load(sys.argv[1])
    names = None
    if len(sys.argv) > 2:
        names = [p["name"] for p in json.load(open(sys.argv[2], encoding="utf-8"))["params"]]
    page = tui["pageData"]
    defs = {d["key"]: d["value"] for d in page["componentDefinitions"]["localComponentDefinitions"]}
    steps = {r["key"]: len(r["value"]) for r in page["replacementStrings"]}
    failures = []

    def check(cond, msg):
        print(("PASS  " if cond else "FAIL  ") + msg)
        if not cond:
            failures.append(msg)

    def handle_of(e):
        remap = {m["key"]: m["value"] for m in e["handle remapping"]["map"]}
        return remap.get("Data", e["componentData"]["data"].get("handleName", "Data"))

    def valid(handle, state):
        """Is a handle valid with the parameters in `state` at those steps (others: any value)?"""
        m = re.fullmatch(r"IndexedEnabling/(\d+)/(\d+)/(Parameter \d+)", handle)
        if m:
            return m.group(3) not in state or int(m.group(1)) == state[m.group(3)]
        m = re.fullmatch(r"NonZero/(Parameter \d+)", handle)
        if m:
            return m.group(1) not in state or state[m.group(1)] != 0
        return handle not in ("Invalid", "Data")

    def drawn(e, state):
        b = e["bounds"]
        if b["showWhenDataModelInvalid"] == "Show":
            return True
        return valid(handle_of(e), state) and all(valid(h, state) for h in b.get("additionalInvalidatingHandles", []))

    tabs = page["tabs"]
    check([t["tabName"] for t in tabs] == ["PLAY", "VOICE"], "tabs are PLAY, VOICE: %s" % [t["tabName"] for t in tabs])
    check([t["fnKeyIndex"] for t in tabs] == [0, 1], "function keys 1-2")
    by_name = {t["tabName"]: t["componentDefinition"]["componentsData"] for t in tabs}
    play, voice, keys_tab = by_name.get("PLAY", []), by_name.get("VOICE", []), by_name.get("KEYS", [])

    kb = None
    for e in play:
        if e["componentData"]["type"] == "kbPanel":
            kb = re.search(r"Parameter \d+", json.dumps(e)).group(0)
    check(kb is not None, "PLAY's keyboard panel is bound to the Keyboard parameter (%s)" % kb)
    check(steps.get(kb) == 2, "Keyboard has two named steps for the visibility rule")

    keyish = {k for k in defs if k.startswith("kb")} - {"kbPanel"}
    for t in tabs:
        items = t["componentDefinition"]["componentsData"]
        for k in (0, 1):
            shown = [e for e in items if drawn(e, {kb: k})]
            has_keys = any(e["componentData"]["type"] in keyish | {"kbPanel"} for e in shown)
            if t["tabName"] == "KEYS":
                check(has_keys, "KEYS shows its keys (Keyboard %d)" % k)
            elif k == 0:
                check(not has_keys, "%s shows no keyboard while Keyboard is off" % t["tabName"])
            elif t["tabName"] == "PLAY":
                check(has_keys, "PLAY shows the keyboard panel while Keyboard is on")

    # every Text label, anywhere, is bound to a real handle
    blank = []

    def walk(items, where):
        for e in items:
            cd = e["componentData"]
            if cd["type"] == "Label" and cd["data"].get("type") == "Text" and cd["data"].get("handleName") in ("Invalid", "", None):
                blank.append("%s/%s" % (where, cd["name"]))
    for t in tabs:
        walk(t["componentDefinition"]["componentsData"], t["tabName"])
    for k, d in defs.items():
        walk(d.get("componentsData", []), k)
    check(not blank, "no Text label is bound to Invalid (they draw nothing on the MPC)%s" % (": " + ", ".join(blank) if blank else ""))

    # Buttons are on top: nothing drawn after a Button overlaps it (any Quality or Keyboard state)
    for tname, items in (("PLAY", play), ("VOICE", voice)):
        covered = []
        for i, e in enumerate(items):
            if e["componentData"]["type"] != "Button":
                continue
            for later in items[i + 1:]:
                if later["componentData"]["type"] == "kbPanel":
                    continue  # the panel covers the page only while open, and the page is hidden then
                if overlap(rect(e), rect(later)):
                    covered.append("%s under %s" % (e["componentData"]["name"], later["componentData"]["name"]))
        check(not covered, "%s: no entry is drawn over a Button%s" % (tname, (": " + "; ".join(covered[:4])) if covered else ""))

    q = "Parameter 18"
    for tname, items in (("PLAY", play), ("VOICE", voice)):
        btns = sorted((rect(e)[0], e["componentData"]["data"]["buttonId"], e["componentData"]["data"]["numButtonsInGroup"])
                      for e in items if e["componentData"]["type"] == "Button" and e["componentData"]["data"]["handleName"] == q)
        caps = []
        for x, step, n in btns:
            lit = [e for e in items if e["componentData"]["type"] == "Label" and rect(e)[0] == x
                   and drawn(e, {q: step, kb: 0}) and e["componentData"]["data"].get("text") in ("LITE", "ECO", "STD", "HIGH")]
            caps.append((lit[0]["componentData"]["data"]["text"] if len(lit) == 1 else "?%d" % len(lit), step, n))
        check(caps == [("LITE", 3, 4), ("ECO", 0, 4), ("STD", 1, 4), ("HIGH", 2, 4)],
              "%s: Quality buttons LITE ECO STD HIGH set 3 0 1 2 of 4, one caption each: %s" % (tname, caps))
        strips = [e for e in items if e["componentData"]["type"] == "Image" and e["componentData"]["data"].get("handleName") == q]
        check(len(strips) == 4 and all(e["componentData"]["data"]["numFrames"] == 4 for e in strips), "%s: each Quality button lights from its own 4-frame strip" % tname)
    check(not any(e["componentData"]["type"] == "radio4" for e in voice), "VOICE's old Eco/Std/High/Lite switch is replaced")

    # the keyboard
    panel = defs["kbPanel"]["componentsData"]
    pd = defs["kbPanel"]
    order = list(pd.keys())
    check(pd.get("version") == 4 and pd.get("hideQLinkBounds") is True
          and order == ["version", "actions", "backgroundData", "ignoreMousePresses", "disableCoarseDataWheel", "hideQLinkBounds", "componentsData"],
          "the keyboard panel hides the Q-Link highlight (version 4, hideQLinkBounds, documented key order): %s" % order)
    for where, items, closes in (("panel", panel, True),):
        kinds = {e["componentData"]["name"]: e for e in items}
        cap = lambda e: defs[e["componentData"]["type"]]["componentsData"][0]["componentData"]["data"]
        done, cancel = kinds.get("Done"), kinds.get("Revert")
        check(done is not None and cap(done).get("text") == "DONE" and cap(done).get("handleName") == "Data",
              "%s: DONE is captioned" % where)
        check(cancel is not None and cap(cancel).get("text") == "CANCEL", "%s: CANCEL is captioned" % where)
        if closes:
            acts = lambda e: [a["handleName"] for a in defs[e["componentData"]["type"]]["actions"]]
            check(acts(done) == ["Data", kb] and acts(cancel) == ["Data", kb], "panel: DONE sings and closes, CANCEL reverts and closes")
        check("Load File" not in kinds, "%s: no Load File key" % where)
        back, clear, space, letter = (rect(kinds[n]) for n in ("Back", "Clear", "Space", "Q"))
        far = abs((back[0] + back[2] / 2) - (clear[0] + clear[2] / 2)) + abs(back[1] - clear[1])
        check(far > 800, "%s: Backspace and Clear All are far apart (%d px)" % (where, far))
        check(space[2] >= 3.5 * letter[2], "%s: Space is %.1f letter keys wide" % (where, space[2] / letter[2]))
        bound = [e["componentData"]["data"].get("handleName") for e in items if e["componentData"]["type"] == "Label"]
        if names:
            want = ["Parameter %d" % names.index(n) for n in ("Lyric Text", "Edit 1", "Edit 2", "Edit 3")]
            check(all(w in bound for w in want), "%s: status and three draft lines are bound (%s)" % (where, want))
        n_keys = sum(1 for e in items if e["componentData"]["type"] in keyish)
        check(n_keys == 52, "%s: 52 keys (every key but Load File): %d" % (where, n_keys))
        tops = [rect(e) for e in items if e["componentData"]["type"] in keyish]
        check(all(r[0] >= 0 and r[0] + r[2] <= 1280 and r[1] + r[3] <= 628 for r in tops), "%s: every key is on screen" % where)

    # PLAY: no file-source controls; the lyric line, the knobs and TYPE are drawn
    play_text = json.dumps(play)
    check("MY WORDS\"" not in play_text and "\"FILE\"" not in play_text and "btn_src" not in play_text,
          "PLAY has no MY WORDS / FILE switch")
    check("START SYLLABLE" not in play_text and "SING: each key" not in play_text and "TYPE LYRICS opens" not in play_text,
          "PLAY has no Start Syllable readout or help text")
    if names:
        sel = "Parameter %d" % names.index("Lyrics")
        check(sel not in play_text, "PLAY has no lyric-set selector")
        start = '"Parameter %d"' % names.index("Start Syl")
        check(start not in play_text and ("Parameter %d\"" % names.index("Start Syl")) not in play_text,
              "PLAY has no Start Syllable control")
        handles = {e["componentData"]["name"]: handle_of(e) for e in play}
        check(handles.get("upcoming") == "Parameter %d" % names.index("Upcoming") and handles.get("live") == "Parameter %d" % names.index("Live Lyr"),
              "PLAY shows the next lyric line and the Live Lyrics switch")
    shown = {e["componentData"]["name"] for e in play if drawn(e, {kb: 0})}
    check({"before", "next", "after", "breath", "type", "restart", "phrase"} <= shown, "PLAY's lyric line, knobs, RESTART and TYPE LYRICS are drawn")

    # the moving head (1.3.3): per body a still base and a strip of the moving region, bound to
    # Tract and shown by Voice; the old six-frame heads are gone; the Animation switch is there
    if names:
        tract, voice_p = "Parameter %d" % names.index("Tract"), "Parameter %d" % names.index("Voice")
        ims = [e for e in play if e["componentData"]["type"] == "Image"]
        strips = [e for e in ims if e["componentData"]["data"].get("handleName") == tract]
        bases = [e for e in ims if e["componentData"]["data"].get("image", "").endswith("_base.png")]
        check(len(strips) == 3 and len(bases) == 3, "PLAY: three bodies' heads, each a base and a moving strip (%d, %d)" % (len(bases), len(strips)))
        for e in strips:
            d = e["componentData"]["data"]
            x, y, w, h = rect(e)
            conds = e["bounds"].get("additionalInvalidatingHandles", [])
            check(d["numFrames"] == 32 and 18 <= x and x + w <= 506 and 68 <= y and y + h <= 394
                  and any(c.startswith("IndexedEnabling/") and c.endswith(voice_p) for c in conds),
                  "%s: 32 frames, inside the head panel, shown by Voice" % d["image"])
        check("head_male.png" not in json.dumps(page) and "\"Head\"" not in play_text, "the old six-frame heads are not used")
        anim = [e for e in play if e["componentData"]["name"] == "animation"]
        check(len(anim) == 1 and handle_of(anim[0]) == "Parameter %d" % names.index("Animation"), "PLAY has the Animation switch")

    # Q-Links
    m = {e["Tab"]: e for e in ql["Screen Mode Q-Links"]["map"]}
    check(sorted(m) == [1, 2], "Q-Links for tabs 1-2: %s" % sorted(m))
    if names:
        pq = m[1]["Q-Links"]
        first = [names[pq["Q-Link %d" % n]] for n in (13, 14, 15, 16)]
        check(first == ["Voice", "Breath", "Breathy", "Rough"], "PLAY Q-Links 13-16: %s" % first)
        rest = {n: names[pq["Q-Link %d" % n]] for n in (9, 10, 12)}
        check(rest == {9: "Vibrato", 10: "Transpose", 12: "Volume"}, "PLAY Q-Links 9, 10, 12: %s" % rest)
        bad = [(t, k, v) for t, e in m.items() for k, v in e["Q-Links"].items() if v != -1 and not (0 <= v < len(names))]
        check(not bad, "every Q-Link names a real parameter")

    images = set(re.findall(r'"(?:image|filmStrip|onImage|offImage)": "([^"]+)"', json.dumps(page)))
    missing = sorted(i for i in images if i and i not in files)
    check(not missing, "every image the skin names is present (%d)%s" % (len(images), (": missing " + ", ".join(missing)) if missing else ""))
    print("FAILED (%d failures)" % len(failures) if failures else "OK (0 failures)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
