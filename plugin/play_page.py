"""The PLAY page: Chimera Voice's performance screen, after the CHIMERA MPC reference.

    CHIMERA  [SING] VOWEL SPEAK BEATBOX                         QUALITY [LITE|ECO|STD|HIGH]
    +-- head (Voice) ---------+  +-- CURRENT PHRASE  SINGING n/m [RESTART] [TYPE LYRICS] --+
    |  CHIMERA's anatomy      |  |   words sung before  [ SYLLABLE ]  words after         |
    |                         |  |   the lyrics' next line                                |
    |                         |  |   [LIVE LYRICS: ON]                         CPU         |
    +-------------------------+  +---------------------------------------------------------+
    BODY  BREATH  BREATHY  ROUGH  VIBRATO  PITCH  (REVERB)  VOLUME      <- Q-Links 13-16, 9-12

1.3.2: the yellow box shows the syllable sounding now while a note sings it ("SINGING n/m"), and
the next one in silence ("NEXT n/m"); the line follows the singing with Live Lyrics on (the
plugin refreshes it at most 5 times a second). No Start Syllable readout or help text.

Built from documented skin features only (docs/ui-reference.md): film strip Images and Knobs,
Labels (drawn under, never over, a touchable entry: a Label takes the touch), Button radio groups, Toggle
Switch actions and conditional visibility. Every item is hidden while the keyboard panel
("Keyboard" parameter) is open. VOWEL, SPEAK and BEATBOX are drawn as
inactive placeholders (they are CHIMERA web app modes this port doesn't have), and the REVERB slot
is unlit: the plugin has no reverb (use an MPC insert effect).

The artwork comes from skin/make_assets.py; the coordinates of the panels and the highlight box
here match the ones drawn into play_bg.png there.
"""
import json
import os

CYAN = "ff46d6e6"
YELLOW = "fff0d23c"
INK = "ff101315"       # text on yellow / cyan
WHITE = "ffffffff"
GREY = "ff7d9399"
DIMTEXT = "ff4c585c"

# Quality's buttons, left to right, with the parameter value each sets (values: Eco 0, Std 1,
# High 2, Lite 3, as saved in projects since 1.2)
QUALITY_ORDER = [("LITE", 3), ("ECO", 0), ("STD", 1), ("HIGH", 2)]

HEAD_X, HEAD_Y = 18, 68   # where the head images sit (their own coordinates start here)
BODIES = ["male", "female", "child"]

ASSETS = (["play_bg.png", "knob_play.png", "knob_play_bi.png", "knob_off.png",
           "btn_mode_on.png", "btn_mode_off.png", "btn_mode_wide_off.png", "btn_restart.png", "btn_type.png", "btn_live.png",
           "clear74x42.png", "clear62x40.png"]
          + ["q74x42_%d.png" % k for k in range(4)] + ["q62x40_%d.png" % k for k in range(4)]
          + ["head_%s_base.png" % b for b in BODIES] + ["head_%s_anim.png" % b for b in BODIES])

SLOT_W, SLOT_Y, SLOT_H = 156, 444, 176


def build(idx, kb, entry, text_style, toggle, panel, quality_selector):
    """Returns (tab, component definitions, Q-Links map entry, asset file names)."""
    P = lambda name: "Parameter %d" % idx[name]
    closed = "IndexedEnabling/0/2/%s" % kb
    items = []

    def add(name, ctype, data, bounds, remap=None, focus="No", show="Hide", when=None):
        # every item: only while the keyboard is closed (and `when`, if given)
        e = entry(name, ctype, data, bounds, remap=remap, focus=focus, show=show, only_when=closed)
        if when:
            e["bounds"]["additionalInvalidatingHandles"].append(when)
        items.append(e)

    def label(name, text_or_handle, bounds, size, colour, just="horizontallyCentred verticallyCentred", style="SemiBold",
              kind="Text", case="Upper Case", font="Titillium Web", when=None):
        data = {"version": 1, "textStyle": text_style(size, colour, just, style=style, case=case, font=font), "type": kind}
        if kind == "Text":
            # bound to a real parameter so the entry is valid (a Text label shows only its text)
            data["handleName"] = kb
            data["text"] = text_or_handle
        else:
            data["handleName"] = text_or_handle
        add(name, "Label", data, bounds, when=when)

    def image(name, file, bounds, frames=None, handle=None, when=None):
        if frames:
            data = {"version": 2, "imageType": "FilmStrip", "colour": "0", "image": file, "numFrames": frames, "handleName": handle}
            add(name, "Image", data, bounds, when=when)
            return
        else:
            data = {"version": 2, "imageType": "Regular", "colour": "0", "image": file}
            # a static image has no parameter: bind its visibility to the keyboard's
            add(name, "Image", data, bounds, show="Show")
            return

    # ---- title row
    label("maker", "GATOR AUDIO", (20, 0, 220, 18), 14, GREY, "left verticallyCentred")
    label("title", "CHIMERA", (16, 12, 250, 50), 44, CYAN, "left verticallyCentred", style="Bold", font="Nexa")
    modes = [("SING", 270, "btn_mode_on.png", True), ("VOWEL", 404, "btn_mode_off.png", False),
             ("SPEAK", 538, "btn_mode_off.png", False), ("BEATBOX", 672, "btn_mode_wide_off.png", False)]
    for name, x, img, live in modes:
        w = 150 if name == "BEATBOX" else 128
        image("mode " + name, img, (x, 12, w, 46))
        if live:
            label("mode " + name, name, (x, 12, w, 46), 26, INK, style="Bold")
        else:
            label("mode " + name, name, (x, 12, w, 32), 22, DIMTEXT)
            label("mode soon", "not in this version", (x, 38, w, 18), 13, DIMTEXT, case="Original", style="Regular")
    label("quality caption", "QUALITY", (826, 10, 122, 28), 18, GREY, "right verticallyCentred")
    label("quality hint", "LITE = low CPU", (806, 36, 142, 22), 14, GREY, "right verticallyCentred", case="Original", style="Regular")
    # the four buttons: their pictures and captions now, the touch-taking Buttons last of all
    # (a Label over a Button takes its taps: 1.3.0-rc2's buttons did nothing)
    q_under, q_top = quality_selector(P("Quality"), 956, 14, 74, 42, 20, "play", only_when=closed)
    items.extend(q_under)

    # ---- the head (1.3.3): CHIMERA's own anatomy, per body (shown by Voice): the still parts, and
    # over them the moving region (tongue, jaw, lips, teeth, velum) as a film strip of the poses
    # skin/make_assets.py drew from the engine's clustered articulations; its frame is "Tract",
    # which the plugin moves to the pose nearest the voice's articulators (Animation on), or rest
    here = os.path.dirname(os.path.abspath(__file__))
    layout = json.load(open(os.path.join(here, "skin", "anim_layout.json"), encoding="utf-8"))
    for b, body in enumerate(BODIES):
        when = "IndexedEnabling/%d/3/%s" % (b, P("Voice"))
        image("head", "head_%s_base.png" % body, (HEAD_X, HEAD_Y, 488, 326), frames=1, handle=P("Voice"), when=when)
        x, y, w, h = layout["bodies"][body]
        image("head moving", "head_%s_anim.png" % body, (HEAD_X + x, HEAD_Y + y, w, h), frames=layout["k"], handle=P("Tract"), when=when)
    label("body", "WithPrefix[BODY / ]/" + P("Voice"), (32, 74, 320, 30), 22, CYAN, "left verticallyCentred", kind="Value", font="Roboto", style="Regular")
    lengths = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "skin", "tract_lengths.json"), encoding="utf-8"))
    for b, body in enumerate(["male", "female", "child"]):
        label("tract", "TRACT %.1f cm" % lengths[body]["rest"], (32, 360, 200, 28), 20, GREY, "left verticallyCentred",
              font="Roboto", style="Regular", case="Original", when="IndexedEnabling/%d/3/%s" % (b, P("Voice")))

    # ---- the words row: the current phrase, where the singer is in it, restart, type
    # (1.3.1: no MY WORDS / FILE switch and no lyric-set selector: typed words are the workflow;
    # a project still on Source = File keeps singing its file, and "Words" says FILE)
    label("phrase", "CURRENT PHRASE", (536, 80, 250, 46), 24, CYAN, "left verticallyCentred", style="Bold")
    label("position", P("Words"), (764, 80, 226, 46), 20, GREY, "right verticallyCentred", kind="Value", case="Original", style="Regular")
    add("restart", "playRestart", {"version": 1, "handleName": "Data"}, (998, 80, 120, 46), remap=[{"key": "Data", "value": P("Restart")}], focus="Yes")
    add("type", "playType", {"version": 1, "handleName": "Data"}, (1124, 80, 132, 46), remap=[{"key": "Data", "value": kb}])

    # ---- the lyric line: sung | THE SYLLABLE (sounding, or next in silence) | to come; then the
    # lyrics' next line (coordinates of the yellow box: NOW_BOX in skin/make_assets.py)
    label("before", P("Before"), (536, 158, 236, 92), 34, GREY, "right verticallyCentred", kind="Value")
    label("next", P("Next"), (780, 150, 240, 108), 58, INK, kind="Value", style="Bold")
    label("after", P("After"), (1028, 158, 228, 92), 34, WHITE, "left verticallyCentred", kind="Value")
    label("upcoming", P("Upcoming"), (540, 288, 716, 50), 30, "ffa9b8bd", "left verticallyCentred", kind="Value")
    # Animation on/off, in the head panel (its text is the plugin's "ANIMATION: ON/OFF")
    add("animation", "playLive", {"version": 1, "handleName": "Data"}, (258, 352, 240, 36), remap=[{"key": "Data", "value": P("Animation")}], focus="Yes")
    # Live Lyrics on/off (a tap flips it; its text is the plugin's "LIVE LYRICS: ON/OFF"), and CPU
    add("live", "playLive", {"version": 1, "handleName": "Data"}, (536, 348, 240, 36), remap=[{"key": "Data", "value": P("Live Lyr")}], focus="Yes")
    label("cpu", "WithPrefix[CPU  ]/" + P("CPU"), (1040, 348, 216, 36), 18, GREY, "right verticallyCentred", kind="Value",
          case="Original", style="Regular")

    # ---- the knob row
    slots = [("BODY", "Voice", "playKnob"), ("BREATH", "Breath", "playKnob"), ("BREATHY", "Breathy", "playKnob"),
             ("ROUGH", "Rough", "playKnob"), ("VIBRATO", "Vibrato", "playKnob"), ("PITCH", "Transpose", "playKnobBi"),
             ("REVERB", None, None), ("VOLUME", "Volume", "playKnob")]
    for i, (cap, param, comp) in enumerate(slots):
        x = 16 + i * SLOT_W
        if param is None:
            image("reverb", "knob_off.png", (x + 26, SLOT_Y, 104, 104))
            label("reverb", cap, (x, SLOT_Y + 104, SLOT_W, 30), 22, DIMTEXT)
            label("reverb", "use MPC FX", (x, SLOT_Y + 134, SLOT_W, 34), 20, DIMTEXT, case="Original", style="Regular")
            continue
        add(cap.lower(), comp, {"version": 1, "handleName": "Data"}, (x, SLOT_Y, SLOT_W, SLOT_H), remap=[{"key": "Data", "value": P(param)}], focus="Yes")
        label(cap.lower(), cap, (x, SLOT_Y + 104, SLOT_W, 30), 22, CYAN)

    tab = {
        "version": 1,
        "tabName": "PLAY",
        "fnKeyIndex": 0,
        "fnKeySubIndex": 0,
        "componentDefinition": {
            "version": 2,
            "actions": [],
            "backgroundData": {"version": 1,
                               "focussed": {"version": 1, "colour": "ff101315", "image": "play_bg.png"},
                               "unfocussed": {"version": 1, "colour": "ff101315", "image": "play_bg.png"}},
            "ignoreMousePresses": False,
            "disableCoarseDataWheel": False,
            "componentsData": items + q_top + [panel],
        },
        "qlinkBoundsData": ["10 440 630 184", "640 440 630 184"],
    }

    ql = {13: "Voice", 14: "Breath", 15: "Breathy", 16: "Rough",
          9: "Vibrato", 10: "Transpose", 12: "Volume",
          5: "Pressed", 6: "Bright", 7: "Ring", 8: "Vib Rate",
          1: "Play Sync", 2: "Start Syl", 3: "Vel Sens", 4: "Fine"}
    qmap = {"Tab": 1, "SubTab": 1, "Bank Direction": "RowTopDown",
            "Q-Links": {"Q-Link %d" % n: (idx[ql[n]] if n in ql else -1) for n in range(1, 17)}}

    def knob_def(strip, overlay):
        return {
            "version": 2,
            "actions": [{"version": 1, "onAction": "Mouse Down", "handler": "Q-Link", "handleName": "Data", "additionalData": ""},
                        {"version": 1, "onAction": "Double Click", "handler": "Show Overlay", "handleName": "Data", "additionalData": overlay},
                        {"version": 1, "onAction": "Enter Pressed", "handler": "Show Overlay", "handleName": "Data", "additionalData": overlay}],
            "backgroundData": {"version": 1, "focussed": {"version": 1, "colour": "0", "image": ""},
                               "unfocussed": {"version": 1, "colour": "0", "image": ""}},
            "ignoreMousePresses": False,
            "disableCoarseDataWheel": False,
            "componentsData": [
                _child("focus", "Focus", {"version": 1, "backgroundColour": "1446d6e6", "outlineColour": CYAN, "backgroundInset": 2.0,
                                          "outlineThickness": 2.0}, (0, 0, SLOT_W, SLOT_H), when="WhenFocussed"),
                _child("knob", "Knob", {"version": 2, "knobType": "FilmStrip", "filmStrip": strip, "numFrames": 64, "invert": False,
                                        "handleName": "Data"}, (26, 0, 104, 104)),
                _child("value", "Label", {"version": 1, "textStyle": text_style(28, WHITE, style="Bold"), "type": "Value", "handleName": "Data"},
                       (0, 134, SLOT_W, 38)),
            ],
        }

    def value_def():
        # drag (or the data wheel) edits the value; double tap opens the knob overlay; the text is
        # a separate Label above it
        return {
            "version": 2,
            "actions": [{"version": 1, "onAction": "Double Click", "handler": "Show Overlay", "handleName": "Data", "additionalData": "knob overlay"},
                        {"version": 1, "onAction": "Enter Pressed", "handler": "Show Overlay", "handleName": "Data", "additionalData": "knob overlay"}],
            "backgroundData": {"version": 1, "focussed": {"version": 1, "colour": "1446d6e6", "image": ""},
                               "unfocussed": {"version": 1, "colour": "0", "image": ""}},
            "ignoreMousePresses": False,
            "disableCoarseDataWheel": False,
            "componentsData": [
                _child("drag", "Knob", {"version": 4, "knobType": "ValueSlider",
                                        "textStyle": text_style(10, "00000000"), "dragOrientation": "Horizontal", "handleName": "Data"},
                       (0, 0, 1, 1), proportional=True),
            ],
        }

    def live_def():
        # a toggle whose caption is its own value text (LIVE LYRICS: ON / OFF), inside it
        d = button_def("btn_live.png", "", CYAN, [toggle("Data")], size=17)
        cap = d["componentsData"][0]["componentData"]["data"]
        cap["type"] = "Value"
        del cap["text"]
        return d

    def button_def(image, caption, ink, actions, size=22):
        return {
            "version": 2,
            "actions": actions,
            "backgroundData": {"version": 1, "focussed": {"version": 1, "colour": "0", "image": image},
                               "unfocussed": {"version": 1, "colour": "0", "image": image}},
            "ignoreMousePresses": False,
            "disableCoarseDataWheel": False,
            "componentsData": [_child("caption", "Label", {"version": 1, "textStyle": text_style(size, ink, style="Bold"), "type": "Text",
                                                           "handleName": "Data", "text": caption}, (0, 0, 1, 1), proportional=True)],
        }

    defs = [{"key": "playKnob", "value": knob_def("knob_play.png", "knob overlay")},
            {"key": "playKnobBi", "value": knob_def("knob_play_bi.png", "knob overlay bipolar")},
            {"key": "playValue", "value": value_def()},
            {"key": "playRestart", "value": button_def("btn_restart.png", "RESTART", CYAN, [toggle("Data")])},
            {"key": "playType", "value": button_def("btn_type.png", "TYPE LYRICS", INK, [toggle(kb)], size=20)},
            {"key": "playLive", "value": live_def()}]
    return tab, defs, qmap, ASSETS


def _child(name, ctype, data, bounds, when="Always", proportional=False):
    return {
        "version": 2,
        "componentData": {"version": 1, "name": name, "type": ctype, "data": data},
        "handle remapping": {"version": 1, "map": []},
        "bounds": {"version": 1, "acceptsHWFocus": "No", "showWhenDataModelInvalid": "Show", "whenVisible": when,
                   "boundsType": "Proportional" if proportional else "Absolute",
                   "bounds": ("%g %g %g %g" % bounds)},
    }
