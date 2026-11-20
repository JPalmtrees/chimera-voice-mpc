// End-to-end test of the KEYS page through the VST2 interface of the native build:
// typing via key toggles (with duplicated reports), Done, the draft label, the state chunk
// round trip into a fresh instance, and that restoring a chunk types nothing. Since 1.2: the
// parameter layout is unchanged with Keyboard appended, Done selects Source = Typed, Quality has
// Lite, and programs leave Quality alone. Since 1.3: the PLAY page's readouts (Before, After,
// Words, Head) are appended and follow the typed words.
//
//   zig c++ -std=c++17 -I<sdk>/framework keyboard_test.cpp -o keyboard_test
//   keyboard_test <path to chimera-voice.dll|.so>      (MPC_PLUGIN_FOLDER: the built plugin folder)
#include "mpc_vst2_abi.h"
#include <chrono>
#include <cstdio>
#include <cstring>
#include <string>
#include <thread>
#include <tuple>
#include <cstdlib>
#include <vector>
#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#endif

static VstIntPtr host(AEffect*, VstInt32 op, VstInt32, VstIntPtr, void*, float) { return op == audioMasterVersion ? 2400 : 0; }
typedef AEffect* (*MainProc)(audioMasterCallback);

static int failures = 0;
#define CHECK(c, ...) do { if (!(c)) { ++failures; std::printf("FAIL  " __VA_ARGS__); std::printf("\n"); } else { std::printf("PASS  " __VA_ARGS__); std::printf("\n"); } } while (0)

static void sleepMs(int ms) { std::this_thread::sleep_for(std::chrono::milliseconds(ms)); }

struct Inst
{
    AEffect* e;
    std::vector<float> l, r;
    explicit Inst(MainProc m) : e(m(host)), l(128), r(128)
    {
        e->dispatcher(e, effSetSampleRate, 0, 0, nullptr, 44100.0f);
        e->dispatcher(e, effSetBlockSize, 0, 128, nullptr, 0.0f);
        e->dispatcher(e, effOpen, 0, 0, nullptr, 0.0f);
        e->dispatcher(e, effMainsChanged, 0, 1, nullptr, 0.0f);
    }
    ~Inst() { e->dispatcher(e, effMainsChanged, 0, 0, nullptr, 0.0f); e->dispatcher(e, effClose, 0, 0, nullptr, 0.0f); }
    int find(const char* name)
    {
        char buf[256];
        for (int i = 0; i < e->numParams; ++i)
        {
            std::memset(buf, 0, sizeof(buf));
            e->dispatcher(e, effGetParamName, i, 0, buf, 0.0f);
            if (std::strcmp(buf, name) == 0) return i;
        }
        return -1;
    }
    std::string display(int i)
    {
        char buf[256] = {0};
        e->dispatcher(e, effGetParamDisplay, i, 0, buf, 0.0f);
        return buf;
    }
    void block() { float* o[2] = {l.data(), r.data()}; e->processReplacing(e, nullptr, o, 128); }
    void settle(int ms = 60) { for (int i = 0; i < ms / 3; ++i) { block(); sleepMs(3); } }
    // a tap: flip the key's toggle (as the skin's Toggle Switch does)
    void tap(int k) { e->setParameter(e, k, e->getParameter(e, k) >= 0.5f ? 0.0f : 1.0f); }
};

// the lyrics editor as the plugin stores it: sung text, draft, cursor (characters), shift
static std::vector<char> editorBytes(const std::string& sung, const std::string& draft, uint32_t cursor)
{
    std::vector<char> b;
    auto u32 = [&](uint32_t v) { for (int i = 0; i < 4; ++i) b.push_back((char)(v >> (8 * i))); };
    u32((uint32_t)sung.size()); b.insert(b.end(), sung.begin(), sung.end());
    u32((uint32_t)draft.size()); b.insert(b.end(), draft.begin(), draft.end());
    u32(cursor);
    b.push_back(0);
    return b;
}

// `chunk` with its lyrics editor (the "VCT2" block at the end) replaced by `ed`
static std::vector<char> withEditor(std::vector<char> chunk, const std::vector<char>& ed)
{
    const std::string s(chunk.begin(), chunk.end());
    const size_t at = s.rfind("VCT2");
    if (at == std::string::npos || at < 4) return {};
    chunk.resize(at - 4);
    auto u32 = [&](uint32_t v) { for (int i = 0; i < 4; ++i) chunk.push_back((char)(v >> (8 * i))); };
    u32((uint32_t)(8 + ed.size()));
    chunk.insert(chunk.end(), {'V', 'C', 'T', '2'});
    u32((uint32_t)ed.size());
    chunk.insert(chunk.end(), ed.begin(), ed.end());
    return chunk;
}

static bool fitsHost(const std::string& s) { return s.size() <= 31; }

int main(int argc, char** argv)
{
    if (argc < 2) { std::printf("usage: keyboard_test <plugin>\n"); return 2; }
#ifdef _WIN32
    HMODULE lib = LoadLibraryA(argv[1]);
    MainProc main_ = lib ? (MainProc)GetProcAddress(lib, "VSTPluginMain") : nullptr;
#else
    void* lib = dlopen(argv[1], RTLD_NOW);
    MainProc main_ = lib ? (MainProc)dlsym(lib, "VSTPluginMain") : nullptr;
#endif
    if (!main_) { std::printf("cannot load %s\n", argv[1]); return 2; }

    std::vector<char> chunk;
    {
        Inst a(main_);
        const int text = a.find("Lyric Text"), lyrics = a.find("Lyrics");
        auto key = [&](const char* n) { int k = a.find(n); if (k < 0) std::printf("no key %s\n", n); return k; };
        a.settle(300);   // the planner thread has read lyrics.txt (on a busy machine, wait up to 2 s more)
        for (int i = 0, nx = a.find("Next"); i < 20 && a.display(nx) == "ah"; ++i) a.settle(100);
        const int ed1 = a.find("Edit 1"), ed2 = a.find("Edit 2"), ed3 = a.find("Edit 3");
        CHECK(text > 0 && lyrics >= 0, "keyboard parameters present (Lyric Text = %d)", text);
        const int quality = a.find("Quality"), source = a.find("Source"), keyboard = a.find("Keyboard");
        const int before = a.find("Before"), after = a.find("After"), words = a.find("Words"), next = a.find("Next"), head = a.find("Head");
        CHECK(quality == 18 && text == 74 && source == 75 && keyboard == 76 && before == 77 && after == 78 && words == 79 && head == 80
              && ed1 == 81 && ed2 == 82 && ed3 == 83 && a.find("Upcoming") == 84 && a.find("Live Lyr") == 85 && a.find("Tract") == 86 && a.find("Animation") == 87 && a.e->numParams == 88,
              "1.2-1.3.2 parameter indices kept, 1.3.3's appended (Quality %d, Lyric Text %d, Source %d, Keyboard %d, Before %d, After %d, Words %d, Head %d, Edit %d-%d, Upcoming %d, Live %d, %d params)",
              quality, text, source, keyboard, before, after, words, head, ed1, ed3, a.find("Upcoming"), a.find("Live Lyr"), a.e->numParams);
        // 1.3.1: a new instance sings typed words (the default phrase, editable)
        CHECK(a.display(source) == "Typed" && a.display(text) == "(sung) 38 chars" && a.display(ed1).rfind("Hello there, I am a voice ", 0) == 0,
              "a new instance starts on typed words: %s / '%s' '%s'", a.display(source).c_str(), a.display(text).c_str(), a.display(ed1).c_str());
        CHECK(a.display(words).rfind("NEXT 1/", 0) == 0 && a.display(next) == "Hel-", "PLAY: '%s', next [%s]", a.display(words).c_str(), a.display(next).c_str());
        CHECK(a.display(a.find("Upcoming")).find("voice") != std::string::npos, "PLAY's next lyric line: '%s'", a.display(a.find("Upcoming")).c_str());
        CHECK(a.display(before).empty() && !a.display(after).empty(), "PLAY's lyric line at the start: '%s' [%s] '%s'",
              a.display(before).c_str(), a.display(next).c_str(), a.display(after).c_str());
        // a project left on Source = File: Done should still sing what was typed
        a.e->setParameter(a.e, source, 1.0f);
        a.settle(30);
        CHECK(a.display(source) == "File", "Source set to File: '%s'", a.display(source).c_str());
        CHECK(a.display(words).rfind("FILE 1 ", 0) == 0, "PLAY says a file is sung: '%s'", a.display(words).c_str());
        // Clear All asks first: one tap keeps the words, the second erases them
        a.tap(key("Clear")); a.settle(170);
        CHECK(a.display(text) == "TAP CLEAR ALL AGAIN TO ERASE" && a.display(ed1).rfind("Hello", 0) == 0,
              "one Clear All tap only asks: '%s' / '%s'", a.display(text).c_str(), a.display(ed1).c_str());
        a.tap(key("Clear")); a.settle(170);
        CHECK(a.display(ed1) == "|" && a.display(text) == "empty: type your words", "the second tap erases: '%s' / '%s'",
              a.display(ed1).c_str(), a.display(text).c_str());
        // "La la!": shift, repeated letters (spaced past the debounce), space, punctuation
        const char* seq[] = {"Shift", "L", "A", "Space", "L", "A", "!"};
        for (const char* n : seq) { a.tap(key(n)); a.settle(170); }
        CHECK(a.display(ed1) == "La la!|" && a.display(text) == "(draft) 6 chars", "typed draft shows: '%s' / '%s'", a.display(ed1).c_str(), a.display(text).c_str());
        // a duplicated report of the same value is not a press
        int b = key("B");
        a.e->setParameter(a.e, b, 1.0f); a.e->setParameter(a.e, b, 1.0f); a.settle(170);
        // a tap the host reports twice (two flips 40 ms apart) is one press
        int c = key("C");
        a.tap(c); sleepMs(40); a.tap(c); a.settle(170);
        CHECK(a.display(ed1) == "La la!bc|", "duplicates collapse: '%s'", a.display(ed1).c_str());
        a.tap(key("Back")); a.settle(170); a.tap(key("Back")); a.settle(170);
        CHECK(a.display(ed1) == "La la!|", "backspace: '%s'", a.display(ed1).c_str());
        CHECK(a.display(lyrics).find("Typed") == std::string::npos, "not sung before Done: '%s'", a.display(lyrics).c_str());
        a.tap(key("Done")); a.settle(200);
        CHECK(a.display(lyrics).find("Typed") != std::string::npos, "Done sings the typed lyrics: '%s'", a.display(lyrics).c_str());
        CHECK(a.display(source) == "Typed" && a.e->getParameter(a.e, source) < 0.5f, "Done selected Source = Typed: '%s'", a.display(source).c_str());
        CHECK(a.display(text) == "(sung) 6 chars", "Done is confirmed on the status line: '%s'", a.display(text).c_str());
        CHECK(a.display(words) == "NEXT 1/2", "PLAY says where in the typed words: '%s'", a.display(words).c_str());
        CHECK(a.display(next) == "La" && a.display(after).find("la") != std::string::npos,
              "PLAY's lyric line shows them: '%s' [%s] '%s'", a.display(before).c_str(), a.display(next).c_str(), a.display(after).c_str());
        // the panel opened and closed again and again (TYPE LYRICS flips Keyboard; CANCEL flips
        // Revert and Keyboard; DONE flips Done and Keyboard, as the skin's actions do)
        bool cycles = true;
        for (int i = 0; i < 5; ++i)
        {
            a.tap(keyboard); a.settle(60);
            a.tap(key("X")); a.settle(170);
            a.tap(key("Revert")); a.tap(keyboard); a.settle(170);
            cycles = cycles && a.display(ed1) == "La la!|" && a.e->getParameter(a.e, keyboard) < 0.5f && a.display(text) == "(sung) 6 chars";
        }
        CHECK(cycles, "five open / type / CANCEL cycles: closed, nothing applied: '%s'", a.display(ed1).c_str());
        a.tap(keyboard); a.settle(60);
        a.tap(key("Space")); a.settle(170); a.tap(key("O")); a.settle(170);
        a.tap(key("Done")); a.tap(keyboard); a.settle(200);
        CHECK(a.e->getParameter(a.e, keyboard) < 0.5f && a.display(text) == "(sung) 8 chars" && a.display(next) == "La",
              "open / type / DONE: closed, applied, sung from the start: '%s' [%s]", a.display(text).c_str(), a.display(next).c_str());
        a.tap(key("Back")); a.settle(170); a.tap(key("Back")); a.settle(170);
        a.tap(key("Done")); a.settle(200);
        CHECK(a.display(text) == "(sung) 6 chars", "back to 'La la!': '%s'", a.display(text).c_str());
        void* data = nullptr;
        VstIntPtr n = a.e->dispatcher(a.e, effGetChunk, 0, 0, &data, 0.0f);
        chunk.assign((char*)data, (char*)data + n);
        CHECK(n > 0 && std::string(chunk.begin(), chunk.end()).find("La la!") != std::string::npos, "the typed lyrics are in the state chunk (%d bytes)", (int)n);
    }
    {
        Inst b(main_);
        const int text = b.find("Lyric Text"), lyrics = b.find("Lyrics");
        b.settle(300);
        CHECK(b.display(text) == "(sung) 38 chars", "a new instance starts with the default words: '%s'", b.display(text).c_str());
        b.e->dispatcher(b.e, effSetChunk, 0, (VstIntPtr)chunk.size(), chunk.data(), 0.0f);
        b.settle(200);
        CHECK(b.display(text) == "(sung) 6 chars" && b.display(b.find("Edit 1")) == "La la!|",
              "reload restores the lyrics (not the default words), and restoring typed nothing: '%s'", b.display(b.find("Edit 1")).c_str());
        CHECK(b.display(lyrics).find("Typed") != std::string::npos, "and sings them: '%s'", b.display(lyrics).c_str());
        void* data = nullptr;
        VstIntPtr n = b.e->dispatcher(b.e, effGetChunk, 0, 0, &data, 0.0f);
        // (the "Lyric Text" display parameter is a refresh counter, so only the lyrics are compared)
        const std::string again((char*)data, (size_t)n), first(chunk.begin(), chunk.end());
        CHECK(again.size() == first.size() && again.substr(again.size() - 14) == first.substr(first.size() - 14), "re-saved chunk carries the same lyrics");
        // programs keep the typed lyrics (they set the voice, not the words) and the Quality
        const int quality = b.find("Quality");
        b.e->setParameter(b.e, quality, 1.0f);
        b.settle(30);
        CHECK(b.display(quality) == "Lite", "Quality's last choice is Lite: '%s'", b.display(quality).c_str());
        b.e->dispatcher(b.e, effSetProgram, 0, 3, nullptr, 0.0f);
        b.settle(100);
        CHECK(b.display(b.find("Edit 1")) == "La la!|", "a program change keeps the draft: '%s'", b.display(b.find("Edit 1")).c_str());
        CHECK(b.display(quality) == "Lite", "and keeps the Quality: '%s'", b.display(quality).c_str());
        // a 1.1 project saved with High: plain 2, normalised 1.0 (of three choices; 1.0 is Lite
        // now). The loader takes the plain value when the two disagree, so it stays High.
        std::vector<char> old = chunk;
        const std::string id = std::string(1, (char)7) + "quality";
        const size_t at = std::string(old.begin(), old.end()).find(id);
        CHECK(at != std::string::npos, "the chunk stores Quality by id");
        if (at != std::string::npos)
        {
            const float plain = 2.0f, norm = 1.0f;
            std::memcpy(&old[at + id.size()], &plain, 4);
            std::memcpy(&old[at + id.size() + 4], &norm, 4);
            Inst c(main_);
            c.settle(100);
            c.e->dispatcher(c.e, effSetChunk, 0, (VstIntPtr)old.size(), old.data(), 0.0f);
            c.settle(100);
            CHECK(c.display(quality) == "High", "a 1.1 project's High loads as High: '%s'", c.display(quality).c_str());
        }
        // the PLAY/VOICE Quality buttons: LITE ECO STD HIGH set the steps 3 0 1 2 of four (a radio
        // Button sets step/3), and each shows its own value
        const char* want[] = {"Lite", "Eco", "Std", "High"};
        const int steps[] = {3, 0, 1, 2};
        bool all = true;
        for (int i = 0; i < 4; ++i)
        {
            b.e->setParameter(b.e, quality, (float)steps[i] / 3.0f);
            b.settle(30);
            all = all && b.display(quality) == want[i];
        }
        CHECK(all, "the four Quality buttons select Lite, Eco, Std and High");
    }
    {
        // a project from before 1.3.1 on Source = File with no typed words: still sings its file
        // (the new default words must not replace it)
        std::vector<char> legacy = withEditor(chunk, editorBytes("", "", 0));
        Inst d(main_);
        d.settle(100);
        const int source = d.find("Source"), words = d.find("Words"), ed1 = d.find("Edit 1");
        const std::string id = std::string(1, (char)6) + "source";
        const size_t at = std::string(legacy.begin(), legacy.end()).find(id);
        CHECK(!legacy.empty() && at != std::string::npos, "built a legacy File project");
        if (at != std::string::npos)
        {
            const float plain = 1.0f, norm = 1.0f;
            std::memcpy(&legacy[at + id.size()], &plain, 4);
            std::memcpy(&legacy[at + id.size() + 4], &norm, 4);
        }
        d.e->dispatcher(d.e, effSetChunk, 0, (VstIntPtr)legacy.size(), legacy.data(), 0.0f);
        d.settle(200);
        CHECK(d.display(source) == "File" && d.display(words).rfind("FILE 1 ", 0) == 0 && d.display(ed1) == "|",
              "a legacy File project keeps its file: %s / '%s' / '%s'", d.display(source).c_str(), d.display(words).c_str(), d.display(ed1).c_str());
    }
    {
        // long, multi-sentence lyrics (with non-ASCII): nothing is cut from what is sung or saved;
        // every screen line fits the host's 31 bytes and the cursor stays on screen
        std::string verse = "Hello there, I am a voice made of air. I sing whatever words you type, line by line, until the "
                            "very last syllable of this rather long second sentence. Then a third one, café au lait, to finish.";
        Inst f(main_);
        f.settle(100);
        const int text = f.find("Lyric Text"), next = f.find("Next"), words = f.find("Words");
        const int ed[3] = {f.find("Edit 1"), f.find("Edit 2"), f.find("Edit 3")};
        std::vector<char> base;
        {
            void* data = nullptr;
            VstIntPtr n = f.e->dispatcher(f.e, effGetChunk, 0, 0, &data, 0.0f);
            base.assign((char*)data, (char*)data + n);
        }
        const uint32_t chars = (uint32_t)verse.size() - 1;   // one two-byte character
        std::vector<char> st = withEditor(base, editorBytes(verse, verse, chars));
        f.e->dispatcher(f.e, effSetChunk, 0, (VstIntPtr)st.size(), st.data(), 0.0f);
        f.settle(300);
        auto lines = [&]() { std::string all; bool fit = true, caret = false;
            for (int k : ed) { std::string l = f.display(k); fit = fit && fitsHost(l); caret = caret || l.find('|') != std::string::npos; all += l + " / "; }
            return std::make_tuple(all, fit, caret); };
        auto [all, fit, caret] = lines();
        char want[48];
        std::snprintf(want, sizeof(want), "(sung) %u chars", chars);
        CHECK(f.display(text) == want, "the whole %u-character text is held: '%s'", chars, f.display(text).c_str());
        CHECK(fit && caret && all.find("finish.|") != std::string::npos, "three lines, each within 31 bytes, cursor at the end: %s", all.c_str());
        // walk the cursor back through the whole text with the < key (one press per character
        // would take a minute at the debounce: the editor's own test walks every character; here
        // 12 presses move it onto an earlier line)
        int lt = f.find("<");
        for (int i = 0; i < 12; ++i) { f.tap(lt); f.settle(170); }
        auto [all2, fit2, caret2] = lines();
        CHECK(fit2 && caret2 && all2.find("finish.|") == std::string::npos, "the cursor moves back and stays on screen: %s", all2.c_str());
        CHECK(f.display(next) == "Hel-" && f.display(words).find("/") != std::string::npos, "sung from the start: [%s] %s",
              f.display(next).c_str(), f.display(words).c_str());
        // saved whole
        void* data = nullptr;
        VstIntPtr n = f.e->dispatcher(f.e, effGetChunk, 0, 0, &data, 0.0f);
        const std::string saved((char*)data, (size_t)n);
        CHECK(saved.find(verse) != std::string::npos, "the project chunk keeps the whole text (%d bytes)", (int)n);
        // and sung to the end: count the syllables by playing them
        int count = 0;
        { const std::string w = f.display(words); count = std::atoi(w.substr(w.find('/') + 1).c_str()); }
        std::string last;
        for (int i = 0; i < count; ++i)
        {
            uint8_t on[3] = {0x90, 60, 100}, off[3] = {0x80, 60, 0};
            for (uint8_t* m : {on, off})
            {
                VstMidiEvent ev{};
                ev.type = kVstMidiType; ev.byteSize = sizeof(ev);
                ev.midiData[0] = (char)m[0]; ev.midiData[1] = (char)m[1]; ev.midiData[2] = (char)m[2];
                struct { VstInt32 n; VstIntPtr reserved; VstEvent* ev[2]; } evs{1, 0, {(VstEvent*)&ev, nullptr}};
                last = m == on ? f.display(next) : last;
                f.e->dispatcher(f.e, effProcessEvents, 0, 0, &evs, 0.0f);
                f.settle(9);
            }
        }
        CHECK(count > 40 && last == "nish", "all %d syllables are sung, the last one '%s'", count, last.c_str());
    }
    std::printf("%s (%d failure%s)\n", failures ? "FAILED" : "OK", failures, failures == 1 ? "" : "s");
    return failures ? 1 : 0;
}
