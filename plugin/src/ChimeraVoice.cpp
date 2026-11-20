// Chimera Voice - an MPC instrument plugin by Gator Audio
//
// CHIMERA's physical-modelling voice (a simulated airway: lungs, vocal folds, pharynx, mouth and
// nose, driven by articulators) singing lyrics from MIDI notes, standalone on the MPC.
//
// Signal flow
//   MIDI note on/off ──► planner thread (Rust): next syllable of lyrics.txt → CHIMERA's sung-note
//                         planner → articulatory gestures
//   gestures ─────────► audio thread (Rust): gestural score → articulators → tract geometry →
//                         airway FDTD + glottis → radiated sound (mono) ──► volume ──► L and R
//   lyrics.txt ───────► planner thread: read from the MPC's drive, words → syllables (English with
//                         the CMU dictionary, pinyin/Mandarin, Korean, Russian)
//   keyboard ─────────► one toggle parameter per key: every flip is a press (cm_key, lock-free) →
//                         planner thread edits the draft; Done sings it; saved in the project chunk
//                         (on the PLAY page's TYPE LYRICS panel, which Done closes, and the KEYS tab)
//   Quality Lite ─────► the airway simulated at 3/4 of the host's rate and resampled up (Rust)
//
// This file is the thin VST2 shell over the SDK framework. Everything about the voice lives in the
// Rust static library (crates/chimera-mpc of the chimera-vocal-synth fork), reached through
// chimera_mpc.h. The audio thread never allocates or locks: the Rust side plans notes, reads files
// and rebuilds voices (body/quality changes) on its own thread.

#include "mpc_plugin.h"
#include "mpc_resources.h"
#include "chimera_mpc.h"

#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>

namespace {

// Parameter order is permanent (saved projects refer to the ids; append new ones at the end).
// Indices 0..18 are shared with the Rust side (crates/chimera-mpc/src/instrument.rs P_*).
enum ParamIndex
{
    kVolume = 0,    // dB, output level
    kVoice,         // Male / Female / Child body (vocal tract, folds, default pitch)
    kTranspose,     // semitones added to every note
    kFine,          // cents
    kBendRange,     // semitones at full pitch bend
    kBreath,        // lung pressure (cmH2O): how hard the singer blows
    kBreathy,       // breathiness: glottal gap, airy tone
    kVibDepth,      // vibrato depth on held vowels (cents)
    kVibRate,       // vibrato rate (Hz)
    kPressed,       // fold tension: lax/soft .. pressed/edgy
    kBright,        // closure sharpness of the glottal pulse
    kRough,         // jitter/shimmer
    kRing,          // epilarynx narrowing: the singer's formant near 3 kHz
    kLyricSet,      // which lyric set of lyrics.txt (sets start with "# Title")
    kStart,         // syllable to start from (restart, play, or turning this knob)
    kRestart,       // toggle: each flip goes back to the start syllable
    kPlaySync,      // restart the lyrics whenever the MPC starts playing
    kVelSens,       // how much key velocity changes the breath
    kQuality,       // Eco / Std / High / Lite: CPU against fidelity (see README; Lite appended in 1.2)
    kNumShared,
    kCpu = kNumShared,   // display only: this instrument's share of the audio deadline
    kNext,               // display only: the syllable the next note sings
    kKeyFirst,           // the on-screen keyboard: one toggle per key (kKeys below)
    kKeyLast = kKeyFirst + 52,
    kText,               // display only: the lyrics editor's draft around its cursor
    kSource,             // Typed (the KEYS page's lyrics, when there are any) / File (lyrics.txt)
    kKeyboard,           // UI only (1.2): the LYRICS page's keyboard panel is open; Done closes it
    kBefore,             // display only (1.3): the words sung before the next syllable
    kAfter,              // display only (1.3): the words after it
    kWords,              // display only (1.3): what is sung: "MY WORDS" (typed) or the lyric set's title
    kHead,               // display only (1.3): the head's frame; held at rest since 1.3.0-rc2
    kEdit1,              // display only (1.3.1): the editor's draft, three lines around the cursor
    kEdit2,
    kEdit3,
    kUpcoming,           // display only (1.3.2): the lyrics' next line, after the words in After
    kLive,               // UI (1.3.2): Off / On: the PLAY lyric line follows the singing
    kTract,              // display only (1.3.3): the head's pose, a frame of skin/head_<body>_anim.png
    kAnim,               // UI (1.3.3): Off / On: the head moves with the voice's articulators
    kNumParams
};

static_assert(kNumShared == 19, "keep in step with the Rust side's P_* indices");
constexpr uint32_t kRustSource = 19;   // P_SOURCE on the Rust side
constexpr uint32_t kRustParams = 20;   // N_PARAMS on the Rust side
constexpr int kHeadFrames = 6;         // frames in skin/head_<body>.png
// Live lyrics (1.3.2): the PLAY lyric line is refreshed when the singer's syllable changes, at most
// this often. The refresh is a few host calls (one per label) made from process(): the MPC offers
// a plugin no other thread to notify it from (no editor idle; screen and knob traffic itself
// arrives on the audio thread), and that path is the one proven on an original MPC One
// (key presses, Done). Text is formatted on the host's UI thread, when it re-reads the labels.
constexpr double kLiveMinSeconds = 0.2;   // at most 5 refreshes a second
// The moving head (1.3.3): the engine picks the pre-drawn pose nearest the articulators about 12
// times a second (on its planner thread); the plugin shows it by moving "Tract" (one host call
// per change, from process like the lyric line), at most this often. Off by default.
constexpr double kAnimMinSeconds = 0.08;  // at most 12.5 head frames a second
constexpr int kTractFrames = 32;          // poses per body: frames in head_<body>_anim.png
// What a new instance sings (1.3.1: typed words are the default; it can be edited or cleared)
constexpr const char* kDefaultWords = "Hello there, I am a voice made of air.";

// The keyboard, in parameter order. Each key is a toggle the skin flips on touch ("Toggle
// Switch"); the caption is the parameter's name. Ids are permanent.
struct Key { const char* id; const char* name; uint8_t code; };
constexpr Key kKeys[] = {
    {"k_1", "1", '1'}, {"k_2", "2", '2'}, {"k_3", "3", '3'}, {"k_4", "4", '4'}, {"k_5", "5", '5'},
    {"k_6", "6", '6'}, {"k_7", "7", '7'}, {"k_8", "8", '8'}, {"k_9", "9", '9'}, {"k_0", "0", '0'},
    {"k_q", "Q", 'q'}, {"k_w", "W", 'w'}, {"k_e", "E", 'e'}, {"k_r", "R", 'r'}, {"k_t", "T", 't'},
    {"k_y", "Y", 'y'}, {"k_u", "U", 'u'}, {"k_i", "I", 'i'}, {"k_o", "O", 'o'}, {"k_p", "P", 'p'},
    {"k_a", "A", 'a'}, {"k_s", "S", 's'}, {"k_d", "D", 'd'}, {"k_f", "F", 'f'}, {"k_g", "G", 'g'},
    {"k_h", "H", 'h'}, {"k_j", "J", 'j'}, {"k_k", "K", 'k'}, {"k_l", "L", 'l'}, {"k_apos", "'", '\''},
    {"k_shift", "Shift", CM_K_SHIFT}, {"k_z", "Z", 'z'}, {"k_x", "X", 'x'}, {"k_c", "C", 'c'}, {"k_v", "V", 'v'},
    {"k_b", "B", 'b'}, {"k_n", "N", 'n'}, {"k_m", "M", 'm'}, {"k_comma", ",", ','}, {"k_period", ".", '.'},
    {"k_back", "Back", CM_K_BACK}, {"k_clear", "Clear", CM_K_CLEAR}, {"k_left", "<", CM_K_LEFT}, {"k_right", ">", CM_K_RIGHT},
    {"k_space", "Space", ' '}, {"k_dash", "-", '-'}, {"k_tilde", "~", '~'}, {"k_quest", "?", '?'}, {"k_excl", "!", '!'},
    {"k_enter", "Enter", CM_K_ENTER},
    {"k_load", "Load File", CM_K_LOAD}, {"k_revert", "Revert", CM_K_REVERT}, {"k_done", "Done", CM_K_DONE},
};
static_assert(sizeof(kKeys) / sizeof(kKeys[0]) == kKeyLast - kKeyFirst + 1, "one parameter per key");

// A host that reports one tap twice (seen on MPC hardware by the OpenMPC project, 30-140 ms
// apart) flips a toggle twice: a second flip of the same key this soon is not a new press.
constexpr double kKeyDebounceSeconds = 0.16;
constexpr uint32_t kExtraMagic = 0x32544356u;   // "VCT2": the lyrics editor in the state chunk

class ChimeraVoice : public mpc::Plugin
{
public:
    ChimeraVoice() : mpc::Plugin(makeInfo())
    {
        addParam(mpc::floatParam("volume", "Volume", -60.0f, 6.0f, 0.0f, "dB", 1));
        addParam(mpc::choiceParam("voice", "Voice", {"Male", "Female", "Child"}, 0));
        addParam(mpc::intParam("transpose", "Transpose", -24, 24, 0, "st"));
        addParam(mpc::intParam("fine", "Fine", -100, 100, 0, "ct"));
        addParam(mpc::intParam("bend_range", "Bend Rng", 0, 12, 2, "st"));
        addParam(mpc::floatParam("breath", "Breath", 2.0f, 22.0f, 7.2f, "cmH2O", 1));
        addParam(mpc::floatParam("breathy", "Breathy", 0.0f, 1.0f, 0.15f, "", 2));
        addParam(mpc::intParam("vib_depth", "Vibrato", 0, 120, 35, "ct"));
        addParam(mpc::floatParam("vib_rate", "Vib Rate", 3.5f, 7.5f, 6.0f, "Hz", 1));
        addParam(mpc::floatParam("pressed", "Pressed", 0.0f, 1.0f, 0.5f, "", 2));
        addParam(mpc::floatParam("bright", "Bright", 0.0f, 1.0f, 0.25f, "", 2));
        addParam(mpc::floatParam("rough", "Rough", 0.0f, 1.0f, 0.1f, "", 2));
        addParam(mpc::floatParam("ring", "Ring", 0.0f, 1.0f, 0.0f, "", 2));
        addParam(mpc::intParam("lyric_set", "Lyrics", 1, 32, 1));
        addParam(mpc::intParam("start", "Start Syl", 1, 999, 1));
        addParam(mpc::boolParam("restart", "Restart", false));
        addParam(mpc::boolParam("play_sync", "Play Sync", true));
        addParam(mpc::floatParam("vel_sens", "Vel Sens", 0.0f, 1.0f, 1.0f, "", 2));
        // "Lite" is appended: projects keep their saved Eco/Std/High (state holds plain values)
        addParam(mpc::choiceParam("quality", "Quality", {"Eco", "Std", "High", "Lite"}, 0));
        addParam(mpc::intParam("cpu", "CPU", 0, 100, 0, "%"));
        addParam(mpc::intParam("next", "Next", 0, 1, 0));
        for (const Key& k : kKeys)
        {
            mpc::Param p = mpc::boolParam(k.id, k.name, false);
            p.automatable = false;
            addParam(p);
        }
        {
            // its value is a fingerprint of what the editor shows (so the host redraws the label
            // when it moves, and a restored project gives back the same value)
            mpc::Param p = mpc::floatParam("lyric_text", "Lyric Text", 0.0f, 1.0f, 0.0f, "", 6);
            p.automatable = false;
            addParam(p);
        }
        addParam(mpc::choiceParam("source", "Source", {"Typed", "File"}, 0));
        {
            // the skin shows the keyboard over the LYRICS page while this is on (TYPE opens it,
            // Done and Close flip it back: both on the host side, no round trip through here)
            mpc::Param p = mpc::boolParam("kb_open", "Keyboard", false);
            p.automatable = false;
            addParam(p);
        }
        // the PLAY page's lyric line: before | next syllable | after. Their values only move (with
        // "Next") so the host re-reads the display strings when the singer moves on.
        for (const char* id : {"lyric_before", "lyric_after", "lyric_words"})
        {
            mpc::Param p = mpc::floatParam(id, id[6] == 'b' ? "Before" : id[6] == 'a' ? "After" : "Words", 0.0f, 1.0f, 0.0f, "", 6);
            p.automatable = false;
            addParam(p);
        }
        {
            // the head's frame in its body's film strip (rest, a, e, i, o, u). Since 1.3.0-rc2 the
            // plugin no longer moves it (no host calls per note), so the head shows rest; the
            // parameter stays for the layout's sake (indices are permanent)
            mpc::Param p = mpc::intParam("head_frame", "Head", 0, kHeadFrames - 1, 0);
            p.automatable = false;
            addParam(p);
        }
        // 1.3.1: the keyboard's draft as three lines (the SDK hands the host at most 31 bytes of a
        // display string, so one line could not show a long draft and its cursor). Values move
        // with the editor's text so the host re-reads them.
        for (const char* id : {"edit_line1", "edit_line2", "edit_line3"})
        {
            mpc::Param p = mpc::floatParam(id, id[9] == '1' ? "Edit 1" : id[9] == '2' ? "Edit 2" : "Edit 3", 0.0f, 1.0f, 0.0f, "", 6);
            p.automatable = false;
            addParam(p);
        }

        {
            mpc::Param p = mpc::floatParam("lyric_upcoming", "Upcoming", 0.0f, 1.0f, 0.0f, "", 6);
            p.automatable = false;
            addParam(p);
        }
        {
            mpc::Param p = mpc::choiceParam("live_lyrics", "Live Lyr", {"Off", "On"}, 1);
            p.automatable = false;
            addParam(p);
        }
        {
            mpc::Param p = mpc::intParam("tract_frame", "Tract", 0, kTractFrames - 1, 0);
            p.automatable = false;
            addParam(p);
        }
        {
            mpc::Param p = mpc::choiceParam("animation", "Animation", {"Off", "On"}, 0);
            p.automatable = false;
            addParam(p);
        }

        // Programs set the voice (Vol .. Vel); they stop before Quality, so choosing one keeps the
        // CPU setting (a program used to put Lite back to Eco), as they keep the lyrics.
        //            Vol  Voice Tr Fine Bend Breath Brthy Vib Rate Press Brt  Rough Ring Set Start Rst Sync Vel
        addProgram("Tenor",        {0, 0, 0, 0, 2, 7.2f, 0.15f, 35, 6.0f, 0.50f, 0.25f, 0.10f, 0.0f, 1, 1, 0, 1, 1.0f});
        addProgram("Soprano",      {0, 1, 0, 0, 2, 7.5f, 0.15f, 40, 5.8f, 0.55f, 0.30f, 0.08f, 0.3f, 1, 1, 0, 1, 1.0f});
        addProgram("Child",        {0, 2, 0, 0, 2, 6.5f, 0.20f, 20, 6.2f, 0.45f, 0.25f, 0.10f, 0.0f, 1, 1, 0, 1, 1.0f});
        addProgram("Airy Whisper", {0, 1, 0, 0, 2, 5.0f, 0.85f, 10, 5.5f, 0.20f, 0.10f, 0.05f, 0.0f, 1, 1, 0, 1, 0.6f});
        addProgram("Opera Bass",   {0, 0, -12, 0, 2, 9.5f, 0.05f, 60, 5.4f, 0.65f, 0.40f, 0.08f, 0.9f, 1, 1, 0, 1, 1.0f});
        addProgram("Belt",         {0, 1, 0, 0, 2, 13.0f, 0.05f, 25, 6.0f, 0.85f, 0.70f, 0.15f, 0.6f, 1, 1, 0, 1, 1.0f});
        addProgram("Rough Rocker", {0, 0, 0, 0, 2, 11.0f, 0.10f, 15, 6.5f, 0.75f, 0.55f, 0.60f, 0.3f, 1, 1, 0, 1, 1.0f});

        createInstrument(44100.0);
        // a new instance starts on typed words (a saved project's own words replace them)
        if (inst_) setWords(kDefaultWords);
    }

    ~ChimeraVoice() override
    {
        if (inst_) cm_free(inst_);
    }

    void prepare(double sampleRate, int maxBlockSize) override
    {
        (void)maxBlockSize;
        // The MPC runs at 44.1 kHz; another rate (a desktop host) needs a voice built for it.
        if (sampleRate > 0.0 && std::fabs(sampleRate - instRate_) > 0.5)
            createInstrument(sampleRate);
    }

    void parameterChanged(int index, float plainValue) override
    {
        if (!inst_) return;
        if (index >= 0 && index < kNumShared)
            cm_set_param(inst_, (uint32_t)index, (double)plainValue);
        else if (index == kSource)
            cm_set_param(inst_, kRustSource, (double)plainValue);
        else if (index == kAnim)
            cm_set_animation(inst_, plainValue >= 0.5f ? 1u : 0u);
        else if (index >= kKeyFirst && index <= kKeyLast)
            keyChanged(index - kKeyFirst, plainValue >= 0.5f);
    }

    void process(mpc::ProcessContext& ctx) override
    {
        const int n = ctx.numFrames;
        float* outL = ctx.outputs[0];
        float* outR = ctx.numOutputs > 1 ? ctx.outputs[1] : nullptr;
        if (!inst_ || n <= 0 || outL == nullptr) return;
        const auto t0 = std::chrono::steady_clock::now();

        cm_transport(inst_, ctx.transport.valid && ctx.transport.playing ? 1u : 0u);
        // Notes are planned on another thread and land within the next block or two, so they
        // are forwarded at the block's start rather than split at their offsets.
        for (int i = 0; i < ctx.numMidiEvents; ++i)
        {
            const mpc::MidiEvent& e = ctx.midi[i];
            if (e.size >= 1)
                cm_midi(inst_, e.data[0], e.size > 1 ? e.data[1] : 0, e.size > 2 ? e.data[2] : 0);
        }
        cm_render(inst_, outL, (size_t)n);
        if (outR) for (int i = 0; i < n; ++i) outR[i] = outL[i];

        // Host notifications from here happen only after a deliberate act, never per note
        // (1.3.0-rc1 notified about 6 parameters at every note; rc2 removed that and 1.3.1 keeps
        // it removed). Each moves the parameters' values so the host re-reads their display text.
        // - the editor's text changed (a key press): its status line and three draft lines
        const uint32_t version = cm_text_version(inst_);
        if (version != shownVersion_.load(std::memory_order_relaxed))
        {
            shownVersion_.store(version, std::memory_order_relaxed);
            const float v = (float)(version % 1000000u) / 1000000.0f;
            for (int p : {kText, kEdit1, kEdit2, kEdit3})
            {
                setNormalised(p, v);
                notifyHostParameterChanged(p);
            }
        }
        // - the PLAY page's lyric line (Next, Before, After, Words, Upcoming: 5 calls):
        //   at once when the lyrics or the place in them changed by Done, Restart, the start
        //   syllable, the source, a restored project or the transport starting (Rust's
        //   jump_version), so it never shows the previous words after Done;
        //   and, with Live Lyrics on, when the syllable sounding changed (Rust's display_version,
        //   moved by the planner thread after a note on or off), at most every 0.2 s. A change
        //   inside that gap is shown at its end (the latest state, not each step).
        sinceRefresh_ += n;
        const uint32_t jump = cm_jump_version(inst_);
        const uint32_t shown = cm_display_version(inst_);
        const bool jumped = jump != shownJump_.load(std::memory_order_relaxed);
        const bool live = getBool(kLive) && shown != shownDisplay_
                          && (double)sinceRefresh_ >= kLiveMinSeconds * sampleRate();
        if (jumped || live)
        {
            shownJump_.store(jump, std::memory_order_relaxed);
            shownDisplay_ = shown;
            sinceRefresh_ = 0;
            ++refreshes_;
            const float v = (float)(refreshes_ % 1000000u) / 1000000.0f;
            setNormalised(kNext, (float)(refreshes_ & 1u));
            notifyHostParameterChanged(kNext);
            for (int p : {kBefore, kAfter, kWords, kUpcoming})
            {
                setNormalised(p, v);
                notifyHostParameterChanged(p);
            }
        }

        // - the head's pose (1 call), when the engine picked another, at most every 0.08 s (the
        //   engine itself picks about every 0.08 s, and only while the animation is on; off, the
        //   pose returns to rest once). The first block shows the engine's pose whatever a
        //   restored project left in "Tract".
        sinceAnim_ += n;
        const uint32_t anim = cm_anim_version(inst_);
        if ((anim != shownAnim_ || !animSynced_) && (double)sinceAnim_ >= kAnimMinSeconds * sampleRate())
        {
            shownAnim_ = anim;
            animSynced_ = true;
            sinceAnim_ = 0;
            const uint32_t f = cm_anim_frame(inst_);
            const float norm = (float)(f < (uint32_t)kTractFrames ? f : 0u) / (float)(kTractFrames - 1);
            if (getNormalised(kTract) != norm)
            {
                setNormalised(kTract, norm);
                notifyHostParameterChanged(kTract);
            }
        }

        // load: render time over the block's real-time length; peak held, then decaying
        const double spent = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
        const double load = spent * sampleRate() / (double)n;
        double avg = loadAvg_.load(std::memory_order_relaxed);
        avg += 0.02 * (load - avg);
        loadAvg_.store(avg, std::memory_order_relaxed);
        double pk = loadPeak_.load(std::memory_order_relaxed) * 0.999;
        loadPeak_.store(load > pk ? load : pk, std::memory_order_relaxed);
    }

    bool formatValue(int index, float plainValue, std::string& out) override
    {
        char buf[96];
        switch (index)
        {
            case kCpu:
                std::snprintf(buf, sizeof(buf), "%d%% pk%d", (int)std::lround(100.0 * loadAvg_.load()), (int)std::lround(100.0 * loadPeak_.load()));
                out = buf;
                return true;
            case kNext:
                if (!inst_) return false;
                cm_text(inst_, 0, 0, buf, sizeof(buf));
                out = buf;
                return true;
            case kText:
                if (!inst_) return false;
                cm_text(inst_, 4, 0, buf, sizeof(buf));
                out = buf;
                return true;
            case kUpcoming:
                if (!inst_) return false;
                cm_text(inst_, 11, 0, buf, sizeof(buf));
                out = buf;
                return true;
            case kLive:
                out = plainValue >= 0.5f ? "LIVE LYRICS: ON" : "LIVE LYRICS: OFF";
                return true;
            case kAnim:
                out = plainValue >= 0.5f ? "ANIMATION: ON" : "ANIMATION: OFF";
                return true;
            case kEdit1:
            case kEdit2:
            case kEdit3:
                if (!inst_) return false;
                cm_text(inst_, 8 + (uint32_t)(index - kEdit1), 0, buf, sizeof(buf));
                out = buf;
                return true;
            case kWords:
            {
                // PLAY's position readout: "SINGING 3/24" while a note sings syllable 3, "NEXT 4/24"
                // in silence (the yellow box shows that one); a project still on a lyrics.txt set
                // (Source = File, kept working from before 1.3.1) says so
                if (!inst_) return false;
                const int set = (int)std::lround(get(kLyricSet));
                char title[64], pos[32], state[16];
                cm_text(inst_, 5, set, title, sizeof(title));
                cm_text(inst_, 3, 0, pos, sizeof(pos));
                cm_text(inst_, 12, 0, state, sizeof(state));
                if (std::string(title) == "Typed")
                    std::snprintf(buf, sizeof(buf), "%s %s", state, pos);
                else
                    std::snprintf(buf, sizeof(buf), "FILE %d %s", set, title);
                out = buf;
                return true;
            }
            case kBefore:
            case kAfter:
                if (!inst_) return false;
                cm_text(inst_, index == kBefore ? 6 : 7, 0, buf, sizeof(buf));
                out = buf;
                return true;
            case kLyricSet:
            {
                if (!inst_) return false;
                const int set = (int)std::lround(plainValue);
                char title[64];
                cm_text(inst_, 5, set, title, sizeof(title));
                std::snprintf(buf, sizeof(buf), "%d %s", set, title);
                out = buf;
                return true;
            }
            case kStart:
            {
                if (!inst_) return false;
                char pos[32];
                cm_text(inst_, 3, 0, pos, sizeof(pos));
                std::snprintf(buf, sizeof(buf), "%d (%s)", (int)std::lround(plainValue), pos);
                out = buf;
                return true;
            }
            default:
                return false;
        }
    }

    // The lyrics editor (typed lyrics, draft, cursor) travels in the project and preset chunk:
    // magic, length, the engine's editor state.
    void saveExtraState(std::vector<uint8_t>& out) override
    {
        if (!inst_) return;
        std::vector<uint8_t> text(cm_state_get(inst_, nullptr, 0));
        size_t n = cm_state_get(inst_, text.data(), text.size());
        if (n != text.size()) { text.resize(n); n = cm_state_get(inst_, text.data(), text.size()); }
        if (n > text.size()) n = 0;   // changed again meanwhile: save none rather than a torn copy
        for (int i = 0; i < 4; ++i) out.push_back((uint8_t)(kExtraMagic >> (8 * i)));
        for (int i = 0; i < 4; ++i) out.push_back((uint8_t)((uint32_t)n >> (8 * i)));
        out.insert(out.end(), text.begin(), text.begin() + (long)n);
    }

    void loadExtraState(const uint8_t* data, size_t size) override
    {
        if (!inst_ || data == nullptr || size < 8) return;
        uint32_t magic = 0, n = 0;
        for (int i = 0; i < 4; ++i) magic |= (uint32_t)data[i] << (8 * i);
        for (int i = 0; i < 4; ++i) n |= (uint32_t)data[4 + i] << (8 * i);
        if (magic != kExtraMagic || (size_t)n > size - 8) return;
        cm_state_set(inst_, data + 8, n);
        // the restored "Lyric Text" value already matches this state: don't move it again
        shownVersion_.store(cm_text_version(inst_), std::memory_order_relaxed);
    }

    // The editor holding `text` as typed, sung and drafted (cm_state_set's format: sung text,
    // draft, cursor in characters, shift).
    void setWords(const char* text)
    {
        const std::string s(text);
        std::vector<uint8_t> b;
        auto u32 = [&](uint32_t v) { for (int i = 0; i < 4; ++i) b.push_back((uint8_t)(v >> (8 * i))); };
        for (int k = 0; k < 2; ++k) { u32((uint32_t)s.size()); b.insert(b.end(), s.begin(), s.end()); }
        u32((uint32_t)s.size());   // ASCII: characters = bytes
        b.push_back(0);
        cm_state_set(inst_, b.data(), b.size());
    }

private:
    // A key parameter moved. Every change of value is one press (a toggle flips each tap), except
    // while a project or program is being restored, a repeated report of the same value, and a
    // second flip of the same key within the debounce window.
    void keyChanged(int k, bool on)
    {
        const bool changed = on != keyState_[k];
        keyState_[k] = on;
        if (!changed || restoringState()) return;
        const double now = std::chrono::duration<double>(std::chrono::steady_clock::now().time_since_epoch()).count();
        if (now - keyTime_[k] < kKeyDebounceSeconds) return;
        keyTime_[k] = now;
        cm_key(inst_, kKeys[k].code);
        // Done means "sing these": a project left on Source = File would otherwise keep singing
        // lyrics.txt, so Done selects Typed (and tells the host, so the switch shows it)
        if (kKeys[k].code == CM_K_DONE && get(kSource) >= 0.5f)
        {
            set(kSource, 0.0f);
            notifyHostParameterChanged(kSource);
        }
    }

    void createInstrument(double sampleRate)
    {
        if (cm_abi_version() != CM_ABI_VERSION || cm_num_params() != kRustParams)
        {
            mpc::log("Chimera Voice: engine library ABI mismatch");
            return;
        }
        // a voice rebuilt for another sample rate keeps the words being typed and sung
        std::vector<uint8_t> words;
        if (inst_)
        {
            words.resize(cm_state_get(inst_, nullptr, 0));
            if (cm_state_get(inst_, words.data(), words.size()) != words.size()) words.clear();
            cm_free(inst_);
            inst_ = nullptr;
        }
        double p[kRustParams];
        for (int i = 0; i < kNumShared; ++i) p[i] = (double)get(i);
        p[kRustSource] = (double)get(kSource);
        inst_ = cm_new(sampleRate, mpc::pluginFolder().c_str(), p);
        instRate_ = sampleRate;
        if (inst_) cm_set_animation(inst_, get(kAnim) >= 0.5f ? 1u : 0u);
        if (cm_anim_frames() != (uint32_t)kTractFrames) mpc::log("Chimera Voice: head poses %u, skin frames %d", cm_anim_frames(), kTractFrames);
        if (inst_ && !words.empty()) cm_state_set(inst_, words.data(), words.size());
    }

    static mpc::PluginInfo makeInfo()
    {
        mpc::PluginInfo info;
        info.name         = "Chimera Voice";
        info.vendor       = "Gator Audio";   // must match plugin.json "manufacturer"
        info.uniqueId     = MPC_FOURCC(kUid[0], kUid[1], kUid[2], kUid[3]);   // must match plugin.json "uid"
        info.version      = 1330;           // 1.3.3.0
        info.isInstrument = true;
        info.numInputs    = 0;
        info.numOutputs   = 2;
        info.tailSamples  = 0;
        return info;
    }

    static constexpr char kUid[5] = "Geqk";

    CmInstrument* inst_ = nullptr;
    double instRate_ = 0.0;
    std::atomic<double> loadAvg_{0.0};
    std::atomic<double> loadPeak_{0.0};
    std::atomic<uint32_t> shownVersion_{0};
    std::atomic<uint32_t> shownJump_{0};
    // the lyric line's refresh (audio thread only)
    uint32_t shownDisplay_ = 0;
    uint64_t sinceRefresh_ = 0;
    uint32_t refreshes_ = 0;
    // the head's refresh (audio thread only)
    uint32_t shownAnim_ = 0;
    bool animSynced_ = false;
    uint64_t sinceAnim_ = 0;

    bool keyState_[kKeyLast - kKeyFirst + 1] = {};
    double keyTime_[kKeyLast - kKeyFirst + 1] = {};
};

} // namespace

MPC_PLUGIN_ENTRY(ChimeraVoice)
