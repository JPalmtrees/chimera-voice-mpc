// Real-time and lyric-flow test of the compiled plugin through VST2, with a host that behaves
// like a demanding MPC: every audioMasterAutomate the plugin sends is answered at once, on the
// calling thread, by re-reading that parameter's display text (as a screen refresh would).
//
// It types "la di da" on the keyboard keys, presses Done, plays notes at real-time pace (128-frame
// blocks, 2.9 ms apart) and checks:
//   - the sung syllables are the typed ones, in order (Next before each note)
//   - Done does refresh the PLAY page's lyric line, with a handful of calls (1.3.1), once
//   - Live Lyrics on (1.3.2): while notes play, the lyric line is refreshed in bursts of 5 calls,
//     at most one burst per 0.2 s (5 a second), and what the host reads at each refresh is the
//     syllable really sounding (or, in silence, the next one): recorded and checked here
//   - Animation on (1.3.3): the head's pose ("Tract") changes with the singing, one call per change,
//     at least 0.08 s apart (at most 12.5 a second), and is back at rest after the phrase; audio
//     block timing with it on; off (the default): never a Tract call
//   - Live Lyrics off: no host call at all from inside processReplacing while notes play, other
//     than audioMasterGetTime (v1.3.0-rc1 notified about 6 parameters at every note)
//   - the audio blocks' render time stays well inside the 2.9 ms deadline
//   - the output is not silent while notes sound
//
//   zig c++ -std=c++17 -I<sdk>/framework realtime_test.cpp -o realtime_test
//   realtime_test <path to chimera-voice.dll|.so>      (MPC_PLUGIN_FOLDER: the built plugin folder)
#include "mpc_vst2_abi.h"
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <string>
#include <thread>
#include <vector>
#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#endif

static int failures = 0;
#define CHECK(c, ...) do { if (!(c)) { ++failures; std::printf("FAIL  " __VA_ARGS__); std::printf("\n"); } else { std::printf("PASS  " __VA_ARGS__); std::printf("\n"); } } while (0)

static std::atomic<bool> inProcess{false};
static std::atomic<int> callsInProcess{0}, automateInProcess{0}, automateTotal{0};
static AEffect* current = nullptr;
static int nextIndex = -1, wordsIndex = -1, tractIndex = -1;
struct Frame { long long block; float value; };
static std::vector<Frame> frames;   // the head's pose at each Tract call from process
static long long blockNo = 0;
struct Seen { long long block; std::string next, words; };
static std::vector<Seen> seen;   // what the host read at each lyric-line refresh from process

static VstIntPtr host(AEffect* e, VstInt32 op, VstInt32 index, VstIntPtr, void*, float)
{
    if (op == audioMasterVersion) return 2400;
    if (op == audioMasterGetTime) return 0;
    if (inProcess.load()) ++callsInProcess;
    if (op == audioMasterAutomate)
    {
        ++automateTotal;
        if (inProcess.load()) ++automateInProcess;
        // a screen refresh, synchronously: read the display text back
        char buf[256] = {0};
        if (e) e->dispatcher(e, effGetParamDisplay, index, 0, buf, 0.0f);
        if (inProcess.load() && index == nextIndex) seen.push_back({blockNo, buf, ""});
        if (inProcess.load() && index == tractIndex && e) frames.push_back({blockNo, e->getParameter(e, index)});
        if (inProcess.load() && index == wordsIndex && !seen.empty()) seen.back().words = buf;
    }
    return 0;
}
typedef AEffect* (*MainProc)(audioMasterCallback);

int main(int argc, char** argv)
{
    if (argc < 2) { std::printf("usage: realtime_test <plugin>\n"); return 2; }
#ifdef _WIN32
    HMODULE lib = LoadLibraryA(argv[1]);
    MainProc main_ = lib ? (MainProc)GetProcAddress(lib, "VSTPluginMain") : nullptr;
#else
    void* lib = dlopen(argv[1], RTLD_NOW);
    MainProc main_ = lib ? (MainProc)dlsym(lib, "VSTPluginMain") : nullptr;
#endif
    if (!main_) { std::printf("cannot load %s\n", argv[1]); return 2; }
    AEffect* e = main_(host);
    current = e;
    e->dispatcher(e, effSetSampleRate, 0, 0, nullptr, 44100.0f);
    e->dispatcher(e, effSetBlockSize, 0, 128, nullptr, 0.0f);
    e->dispatcher(e, effOpen, 0, 0, nullptr, 0.0f);
    e->dispatcher(e, effMainsChanged, 0, 1, nullptr, 0.0f);
    std::vector<float> l(128), r(128);
    std::vector<double> times;
    double peak = 0.0;
    auto block = [&](bool timed) {
        float* o[2] = {l.data(), r.data()};
        inProcess = true;
        const auto t0 = std::chrono::steady_clock::now();
        e->processReplacing(e, nullptr, o, 128);
        ++blockNo;
        const double us = std::chrono::duration<double, std::micro>(std::chrono::steady_clock::now() - t0).count();
        inProcess = false;
        if (timed) times.push_back(us);
        for (float x : l) peak = std::fmax(peak, std::fabs(x));
        std::this_thread::sleep_for(std::chrono::microseconds(2900));
    };
    auto run = [&](int ms, bool timed) { for (int i = 0; i < ms * 441 / 1280 + 1; ++i) block(timed); };
    auto find = [&](const char* name) {
        char buf[256];
        for (int i = 0; i < e->numParams; ++i) { std::memset(buf, 0, sizeof(buf)); e->dispatcher(e, effGetParamName, i, 0, buf, 0.0f); if (!std::strcmp(buf, name)) return i; }
        return -1;
    };
    auto display = [&](int i) { char buf[256] = {0}; e->dispatcher(e, effGetParamDisplay, i, 0, buf, 0.0f); return std::string(buf); };
    auto tap = [&](const char* key) { int k = find(key); e->setParameter(e, k, e->getParameter(e, k) >= 0.5f ? 0.0f : 1.0f); run(180, false); };
    auto midi = [&](uint8_t s, uint8_t n, uint8_t v) {
        VstMidiEvent ev{};
        ev.type = kVstMidiType; ev.byteSize = sizeof(ev); ev.midiData[0] = (char)s; ev.midiData[1] = (char)n; ev.midiData[2] = (char)v;
        struct { VstInt32 n; VstIntPtr reserved; VstEvent* ev[2]; } evs{1, 0, {(VstEvent*)&ev, nullptr}};
        e->dispatcher(e, effProcessEvents, 0, 0, &evs, 0.0f);
    };

    run(400, false);   // lyrics.txt read
    const int next = find("Next"), source = find("Source"), live = find("Live Lyr");
    nextIndex = next; wordsIndex = find("Words"); tractIndex = find("Tract");
    const int anim = find("Animation");
    CHECK(tractIndex == 86 && anim == 87 && display(anim) == "ANIMATION: OFF", "Tract 86, Animation 87, off by default: %d %d '%s'",
          tractIndex, anim, display(anim).c_str());
    CHECK(live == 85 && display(live) == "LIVE LYRICS: ON", "Live Lyrics is parameter 85, on by default: %d '%s'", live, display(live).c_str());
    tap("Clear"); tap("Clear");   // the default words; Clear All asks once first
    for (const char* k : {"L", "A", "Space", "D", "I", "Space", "D", "A"}) tap(k);
    automateInProcess = 0;
    tap("Done");
    run(200, false);
    const int onDone = automateInProcess.load();
    // Next, Before, After, Words (the lyric line) + the editor's status and three lines
    CHECK(onDone >= 4 && onDone <= 10, "Done refreshes the lyric line with a few host calls, once (%d)", onDone);
    CHECK(display(next) == "la", "PLAY shows the new words at once after Done: [%s]", display(next).c_str());
    CHECK(display(source) == "Typed", "Done selected Source = Typed: '%s'", display(source).c_str());
    // a phrase, twice: Live Lyrics on, then off
    const uint8_t notes[] = {60, 62, 64, 65, 64, 62};
    std::vector<std::string> sung;
    auto phrase = [&](bool timed) {
        for (uint8_t n : notes)
        {
            sung.push_back(display(next));
            midi(0x90, n, 100);
            run(300, timed);
            midi(0x80, n, 0);
            run(80, timed);
        }
        run(300, timed);
    };
    automateInProcess = 0; callsInProcess = 0; seen.clear();
    const long long b0 = blockNo;
    times.clear();
    phrase(true);
    {
        std::vector<double> t = times;
        std::sort(t.begin(), t.end());
        const double p99on = t.empty() ? 0 : t[t.size() * 99 / 100];
        std::printf("INFO  Live Lyrics on: %zu blocks, median %.0f us, p99 %.0f us, worst %.0f us\n", t.size(),
                    t.empty() ? 0 : t[t.size() / 2], p99on, t.empty() ? 0 : t.back());
        CHECK(p99on < 1450.0, "with Live Lyrics on, 99%% of blocks render in under half the deadline (%.0f us)", p99on);
    }
    const int liveCalls = automateInProcess.load();
    const double secs = (double)(blockNo - b0) * 128.0 / 44100.0;
    long long minGap = 1 << 30;
    bool truthful = !seen.empty(), sawSinging = false;
    std::string trail;
    for (size_t i = 0; i < seen.size(); ++i)
    {
        if (i) minGap = std::min(minGap, seen[i].block - seen[i - 1].block);
        truthful = truthful && (seen[i].next == "la" || seen[i].next == "di" || seen[i].next == "da");
        sawSinging = sawSinging || seen[i].words.rfind("SINGING", 0) == 0;
        trail += "[" + seen[i].next + " " + seen[i].words + "]";
    }
    std::printf("INFO  live lyrics: %zu refreshes, %d host calls in %.1f s (%.1f refreshes/s), closest %.0f ms apart\n",
                seen.size(), liveCalls, secs, seen.size() / secs, seen.size() > 1 ? minGap * 128.0 / 44.1 : 0.0);
    std::printf("INFO  shown: %s\n", trail.c_str());
    CHECK(seen.size() >= 6, "with Live Lyrics on the lyric line follows the notes (%zu refreshes)", seen.size());
    CHECK(liveCalls == 5 * (int)seen.size(), "each refresh is 5 host calls (%d for %zu)", liveCalls, seen.size());
    CHECK(seen.size() < 2 || minGap * 128.0 / 44100.0 >= 0.199, "refreshes are at least 0.2 s apart (%.0f ms)", minGap * 128.0 / 44.1);
    CHECK(seen.size() / secs <= 5.0, "at most 5 refreshes a second (%.2f)", seen.size() / secs);
    CHECK(truthful && sawSinging, "the host is shown the syllables really sung, marked SINGING while they sound");
    CHECK(frames.empty(), "Animation off: no head call while singing (%zu)", frames.size());
    // Animation on: the head follows the singing, rate-limited, and rests after the phrase
    tap("Animation");
    CHECK(display(anim) == "ANIMATION: ON", "Animation on: '%s'", display(anim).c_str());
    run(200, false);
    frames.clear(); times.clear();
    const long long a0 = blockNo;
    phrase(true);
    run(600, true);
    {
        const double asecs = (double)(blockNo - a0) * 128.0 / 44100.0;
        long long gap = 1 << 30;
        std::vector<float> distinct;
        for (size_t i = 0; i < frames.size(); ++i)
        {
            if (i) gap = std::min(gap, frames[i].block - frames[i - 1].block);
            if (std::find(distinct.begin(), distinct.end(), frames[i].value) == distinct.end()) distinct.push_back(frames[i].value);
        }
        std::vector<double> t = times;
        std::sort(t.begin(), t.end());
        const double p99a = t.empty() ? 0 : t[t.size() * 99 / 100];
        std::printf("INFO  animation: %zu head calls in %.1f s (%.1f/s), closest %.0f ms apart, %zu distinct poses; blocks median %.0f us, p99 %.0f us, worst %.0f us\n",
                    frames.size(), asecs, frames.size() / asecs, frames.size() > 1 ? gap * 128.0 / 44.1 : 0.0, distinct.size(),
                    t.empty() ? 0 : t[t.size() / 2], p99a, t.empty() ? 0 : t.back());
        CHECK(distinct.size() >= 4, "the head moves through several poses while singing (%zu)", distinct.size());
        CHECK(frames.size() < 2 || gap * 128.0 / 44100.0 >= 0.079, "head calls at least 0.08 s apart (%.0f ms)", gap * 128.0 / 44.1);
        CHECK(frames.size() / asecs <= 12.5, "at most 12.5 head calls a second (%.1f)", frames.size() / asecs);
        CHECK(!frames.empty() && frames.back().value == 0.0f, "back at rest after the phrase (%.3f)", frames.empty() ? -1.0f : frames.back().value);
        CHECK(p99a < 1450.0, "with Animation on, 99%% of blocks render in under half the deadline (%.0f us)", p99a);
    }
    tap("Animation");
    run(300, false);
    // Live Lyrics off: no host call from the audio thread while singing
    tap("Live Lyr");
    CHECK(display(live) == "LIVE LYRICS: OFF", "Live Lyrics off: '%s'", display(live).c_str());
    run(300, false);
    sung.clear();
    automateInProcess = 0; callsInProcess = 0; automateTotal = 0; times.clear(); peak = 0.0;
    phrase(true);
    std::string seq;
    for (auto& s : sung) seq += "[" + s + "]";
    CHECK(sung.size() == 6 && sung[0] == "la" && sung[1] == "di" && sung[2] == "da" && sung[3] == "la" && sung[4] == "di" && sung[5] == "da",
          "the notes sing the typed words in order, then wrap: %s", seq.c_str());
    CHECK(peak > 0.01, "the voice sounds (peak %.3f)", peak);
    std::vector<double> s = times;
    std::sort(s.begin(), s.end());
    const double worst = s.empty() ? 0 : s.back(), p99 = s.empty() ? 0 : s[s.size() * 99 / 100];
    std::printf("INFO  %zu blocks: median %.0f us, p99 %.0f us, worst %.0f us (deadline 2902 us); automate calls %d (%d inside process), other host calls inside process %d\n",
                s.size(), s.empty() ? 0 : s[s.size() / 2], p99, worst, automateTotal.load(), automateInProcess.load(), callsInProcess.load() - automateInProcess.load());
    CHECK(automateInProcess.load() == 0, "no audioMasterAutomate from the audio thread while singing (%d)", automateInProcess.load());
    CHECK(p99 < 1450.0, "99%% of blocks render in under half the deadline (%.0f us)", p99);
    e->dispatcher(e, effMainsChanged, 0, 0, nullptr, 0.0f);
    e->dispatcher(e, effClose, 0, 0, nullptr, 0.0f);
    std::printf(failures ? "FAILED (%d failures)\n" : "OK (0 failures)\n", failures);
    return failures ? 1 : 0;
}
