#!/usr/bin/env python3
"""Adds the on-screen keyboard for typing lyrics to the generated Plugin Skins, and the PLAY page:
a keyboard panel over PLAY (TYPE LYRICS opens it; DONE sings the draft and closes it; CANCEL
throws the draft away and closes it).

    skins_post.py <Plugin Skins dir> <params.json>      (run by tools/build.py after gen_ui.py)

Tabs (1.3.2): PLAY, VOICE. The LYRICS tab of 1.1-1.3.0 and the KEYS tab of 1.1-1.3.1 are gone
(the panel on PLAY is the keyboard); every key parameter, the lyrics.txt engine and its parameters
stay, so older projects load as saved.

The panel hides the MPC's Q-Link highlight while it is open (hideQLinkBounds, ui-reference 4.1):
PLAY's highlight rectangles sit over the knob row, which is where SHIFT and SPACE are when the
keyboard covers the page (seen on an original MPC One with 1.3.1-rc1).

The keyboard uses only documented skin features (docs/ui-reference.md): each key is a custom
component whose Mouse Down action is "Toggle Switch" on that key's own parameter, captioned by a
Label inside it. The draft is shown by Value labels bound to the "Lyric Text" (status) and
"Edit 1..3" (three lines around the cursor) parameters, whose display strings the plugin supplies.

Hardware lessons this file follows (original MPC One):
- an entry with showWhenDataModelInvalid "Show" is always drawn: anything conditional is "Hide"
  with its own handle valid only when it should show (1.3.0-rc1 drew the keyboard over PLAY);
- a Text label bound to "Invalid" draws nothing (1.3.0-rc2's DONE and BACK keys were blank): every
  caption is bound to a real parameter;
- a touch goes to the topmost entry under the finger: a Label over a Button swallows its taps
  (1.3.0-rc2's PLAY Quality buttons did nothing), so Buttons are the last, topmost entries, their
  captions drawn underneath.
"""
import json
import os
import shutil
import sys

import play_page

LEFT = 20
WIDTH = 1240
TOP = 172         # first key row (tab coordinates: the tab area is 1280 x 628)
ROW_H = 90
GAP = 8
UNITS = 12.0      # each key row is 12 units wide

INK = "ff101315"
KEY = "ff222a2e"
CONTROL = "ff1d3a40"
YELLOW = "fff0d23c"
RED = "ff5a2228"
CYAN = "ff46d6e6"

# rows: (parameter name, caption or None for the parameter's own name, width in units, colour)
ROWS = [
    [(c, None, 1, KEY) for c in "1234567890"] + [("Back", "BACKSPACE", 2, CONTROL)],
    [(c, None, 1, KEY) for c in "QWERTYUIOP"] + [("'", None, 1, KEY), ("?", None, 1, KEY)],
    [(c, None, 1, KEY) for c in "ASDFGHJKL"] + [("-", None, 1, KEY), ("Enter", "ENTER", 2, CONTROL)],
    [("Shift", "SHIFT", 2, CONTROL)] + [(c, None, 1, KEY) for c in "ZXCVBNM"]
    + [(",", None, 1, KEY), (".", None, 1, KEY), ("!", None, 1, KEY)],
    # the bottom row: Clear All far from Backspace (and it asks before erasing), a wide Space
    [("Clear", "CLEAR ALL", 1.4, RED), ("~", None, 0.8, KEY), ("<", None, 0.9, CONTROL), ("Space", "SPACE", 4.3, CONTROL),
     (">", None, 0.9, CONTROL), ("Revert", "CANCEL", 1.6, CONTROL), ("Done", "DONE", 2.1, YELLOW)],
]


def text_style(height, colour, just="horizontallyCentred verticallyCentred", style="Regular", case="Original", font="Titillium Web"):
    return {"version": 1, "font": {"version": 1, "name": font, "style": style, "height": float(height)},
            "colour": colour, "justification": just, "case": case}


def toggle(handle):
    return {"version": 1, "onAction": "Mouse Down", "handler": "Toggle Switch", "handleName": handle, "additionalData": ""}


def key_def(colour, also=(), caption=None, ink="ffffffff", size=30):
    """A key: touching it flips its Data handle (and any handles in `also`); captioned with the
    bound parameter's name, or with fixed text. The caption is bound to Data (a real parameter:
    a Text label on "Invalid" is not drawn on the MPC)."""
    style = text_style(size, ink, style="SemiBold")
    label = ({"version": 1, "textStyle": style, "type": "Name", "handleName": "Data"} if caption is None
             else {"version": 1, "textStyle": style, "type": "Text", "handleName": "Data", "text": caption})
    return {
        "version": 2,
        "actions": [toggle("Data")] + [toggle(h) for h in also],
        "backgroundData": {"version": 1,
                           "focussed": {"version": 1, "colour": colour, "image": ""},
                           "unfocussed": {"version": 1, "colour": colour, "image": ""}},
        "ignoreMousePresses": False,
        "disableCoarseDataWheel": False,
        "componentsData": [{
            "version": 2,
            "componentData": {"version": 1, "name": "caption", "type": "Label", "data": label},
            "handle remapping": {"version": 1, "map": []},
            "bounds": {"version": 1, "acceptsHWFocus": "No", "showWhenDataModelInvalid": "Show", "whenVisible": "Always",
                       "boundsType": "Proportional", "bounds": "0 0 1 1"},
        }],
    }


def entry(name, ctype, data, bounds, remap=None, focus="No", show="Show", only_when=None):
    b = {"version": 1, "acceptsHWFocus": focus, "showWhenDataModelInvalid": show, "whenVisible": "Always",
         "boundsType": "Absolute", "bounds": "%d %d %d %d" % bounds}
    if only_when:
        b["version"] = 2
        b["additionalInvalidatingHandles"] = [only_when]
    return {
        "version": 2,
        "componentData": {"version": 1, "name": name, "type": ctype, "data": data},
        "handle remapping": {"version": 1, "map": remap or []},
        "bounds": b,
    }


def panel_entry(kb):
    """The keyboard panel over a page, hidden by its OWN handle: valid only while Keyboard is at
    step 1 ("Hide" hides it otherwise; "Show" would draw it always)."""
    open_ = "IndexedEnabling/1/2/%s" % kb
    return entry("keyboard panel", "kbPanel", {"version": 1, "handleName": "Data"}, (0, 0, 1280, 628),
                 remap=[{"key": "Data", "value": open_}], focus="Children", show="Hide", only_when=open_)


def container(children, colour, hide_qlink=False):
    c = {
        "version": 2,
        "actions": [],
        "backgroundData": {"version": 1,
                           "focussed": {"version": 1, "colour": colour, "image": ""},
                           "unfocussed": {"version": 1, "colour": colour, "image": ""}},
        "ignoreMousePresses": False,
        "disableCoarseDataWheel": False,
    }
    if hide_qlink:
        # version 4 (ui-reference 4.1): no Q-Link highlight while this is shown; key order matters
        c["version"] = 4
        c["hideQLinkBounds"] = True
    c["componentsData"] = children
    return c


def label(name, handle, kind, text, bounds, size, colour, just="left verticallyCentred", style="Regular"):
    data = {"version": 1, "textStyle": text_style(size, colour, just, style=style), "type": kind, "handleName": handle}
    if kind == "Text":
        data["text"] = text
    return entry(name, "Label", data, bounds)


def keyboard(idx, panel, defs):
    """The text area (status line and three draft lines) and the five key rows. `panel`: the
    PLAY page's overlay (DONE and CANCEL also close it) rather than the KEYS tab."""
    P = lambda n: "Parameter %d" % idx[n]
    hint = ("DONE sings these words from the start.   CANCEL drops your changes." if panel
            else "DONE sings these words.   CANCEL puts back the words being sung.")
    out = [
        label("status", P("Lyric Text"), "Value", None, (LEFT, 6, 600, 34), 24, CYAN, style="SemiBold"),
        label("hint", P("Lyric Text"), "Text", hint, (LEFT + 600, 6, WIDTH - 600, 34), 20, "ff7d9399", "right verticallyCentred"),
    ]
    for i in range(3):
        out.append(label("line %d" % (i + 1), P("Edit %d" % (i + 1)), "Value", None, (LEFT + 10, 42 + i * 42, WIDTH - 20, 42), 36, "ffffffff"))
    for r, row in enumerate(ROWS):
        y = TOP + r * ROW_H
        x = float(LEFT)
        for name, caption, units, colour in row:
            w = WIDTH * units / UNITS
            if name == "Done":
                ctype = "kbDoneClose" if panel else "kbDone"
            elif name == "Revert":
                ctype = "kbCancel" if panel else "kbRevert"
            elif caption is None:
                ctype = "kbKey" if colour == KEY else "kbControl"
            else:
                ctype = "kb_" + name.lower()
                if ctype not in {d["key"] for d in defs}:
                    defs.append({"key": ctype, "value": key_def(colour, caption=caption, size=26)})
            out.append(entry(name, ctype, {"version": 1, "handleName": "Data"},
                             (round(x + GAP / 2), y, round(w - GAP), ROW_H - GAP),
                             remap=[{"key": "Data", "value": P(name)}], show="Hide"))
            x += w
    return out


def quality_selector(q, x0, y0, w, h, size, prefix, only_when=None):
    """Four Quality buttons in the order LITE ECO STD HIGH (the parameter's values stay Eco 0,
    Std 1, High 2, Lite 3, for saved projects). Under each: a film strip of the four values (lit on
    its own step) and its caption, dark on yellow when selected; on top: a Button with a clear
    image, which takes the touch. Returns (under, on top) entry lists."""
    under, top = [], []

    def add(lst, e, cond=None):
        for c in (cond, only_when):
            if c:
                e["bounds"]["version"] = 2
                e["bounds"].setdefault("additionalInvalidatingHandles", []).append(c)
        lst.append(e)

    for pos, (cap, step) in enumerate(play_page.QUALITY_ORDER):
        x = x0 + pos * (w + 4)
        add(under, entry("%s quality %s" % (prefix, cap), "Image",
                         {"version": 2, "imageType": "FilmStrip", "colour": "0", "image": "q%dx%d_%d.png" % (w, h, step),
                          "numFrames": 4, "handleName": q}, (x, y0, w, h), show="Hide"))
        for v in range(4):
            add(under, entry("%s quality %s" % (prefix, cap), "Label",
                             {"version": 1, "textStyle": text_style(size, INK if v == step else CYAN, style="Bold"), "type": "Text",
                              "handleName": q, "text": cap}, (x, y0, w, h), show="Hide"), "IndexedEnabling/%d/4/%s" % (v, q))
        add(top, entry("%s quality %s" % (prefix, cap), "Button",
                       {"version": 2, "onImage": "clear%dx%d.png" % (w, h), "offImage": "clear%dx%d.png" % (w, h), "buttonId": step,
                        "numButtonsInGroup": 4, "handleName": q, "gestureBehaviour": "Instant"}, (x, y0, w, h), focus="Yes", show="Hide"))
    return under, top


def main():
    skins, params_path = sys.argv[1], sys.argv[2]
    params = json.load(open(params_path, encoding="utf-8"))["params"]
    names = [p["name"] for p in params]
    idx = {n: i for i, n in enumerate(names)}
    for n in ("Lyric Text", "Edit 1", "Edit 2", "Edit 3", "Keyboard", "Revert", "Done", "Clear", "Back", "Space"):
        assert n in idx, n
    kb = "Parameter %d" % idx["Keyboard"]
    q = "Parameter %d" % idx["Quality"]

    tui_path = os.path.join(skins, "TUI.json")
    tui = json.load(open(tui_path, encoding="utf-8"))
    page = tui["pageData"]
    defs = page["componentDefinitions"]["localComponentDefinitions"]
    defs[:] = [d for d in defs if not d["key"].startswith("kb")]
    defs += [{"key": "kbKey", "value": key_def(KEY)},
             {"key": "kbControl", "value": key_def(CONTROL)},
             # DONE: sing (flip its own key) and close (flip Keyboard)
             {"key": "kbDoneClose", "value": key_def(YELLOW, also=[kb], caption="DONE", ink=INK, size=34)},
             # CANCEL: put the sung words back in the draft (Revert) and close
             {"key": "kbCancel", "value": key_def(CONTROL, also=[kb], caption="CANCEL", size=28)}]
    rs = page.setdefault("replacementStrings", [])
    rs[:] = [r for r in rs if r["key"] != kb] + [{"key": kb, "value": ["Off", "On"]}]

    defs.append({"key": "kbPanel", "value": container(keyboard(idx, True, defs), INK, hide_qlink=True)})
    tabs = [t for t in page["tabs"] if t.get("tabName") not in ("KEYS", "LYRICS")]
    assert [t["tabName"] for t in tabs] == ["VOICE"], [t["tabName"] for t in tabs]

    # VOICE: its Quality control becomes the same LITE ECO STD HIGH buttons as on PLAY
    voice = tabs[0]["componentDefinition"]["componentsData"]
    qe = next(e for e in voice if any(m["value"] == q for m in e["handle remapping"]["map"]))
    x, y, w, h = [int(float(v)) for v in qe["bounds"]["bounds"].split()]
    voice.remove(qe)
    voice.append(label("quality name", q, "Name", None, (x + 15, y + 8, 110, 34), 24, "ff6a6a6c", "left"))
    voice.append(label("quality hint", q, "Text", "LITE = lowest CPU", (x + 120, y + 8, w - 135, 34), 18, "ff6a6a6c", "right verticallyCentred"))
    under, top = quality_selector(q, x + 15, y + 54, 62, 40, 18, "voice")
    voice += under + top

    # the PLAY page, first of all (the other tabs move one function key along)
    play_tab, play_defs, play_q, assets = play_page.build(idx, kb, entry, text_style, toggle, panel_entry(kb), quality_selector)
    defs[:] = [d for d in defs if d["key"] not in {x["key"] for x in play_defs}] + play_defs
    for t in tabs:
        t["fnKeyIndex"] += 1
    page["tabs"] = [play_tab] + tabs
    here = os.path.dirname(os.path.abspath(__file__))
    for a in assets:
        shutil.copy2(os.path.join(here, "skin", a), os.path.join(skins, a))
    with open(tui_path, "w", encoding="utf-8") as fh:
        json.dump(tui, fh, indent=1)

    # Q-Links: PLAY is tab 1, gen_ui's VOICE moves to 2
    q_path = os.path.join(skins, "Q-Links.json")
    ql = json.load(open(q_path, encoding="utf-8"))
    m = [e for e in ql["Screen Mode Q-Links"]["map"] if e["Tab"] == 1]
    for e in m:
        e["Tab"] = 2
    ql["Screen Mode Q-Links"]["map"] = [play_q] + m
    with open(q_path, "w", encoding="utf-8") as fh:
        json.dump(ql, fh, indent=1)
    print("  PLAY, VOICE: keyboard panel on PLAY (Parameter %d), draft lines on Parameters %d-%d"
          % (idx["Keyboard"], idx["Edit 1"], idx["Edit 3"]))


if __name__ == "__main__":
    main()
