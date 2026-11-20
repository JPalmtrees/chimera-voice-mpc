// C ABI of the CHIMERA engine for the MPC (crates/chimera-mpc in the chimera-vocal-synth fork).
// The Rust static library linked into this plugin implements these.
//
// Threads: cm_render, cm_midi and cm_transport from the audio thread only; cm_set_param from any
// thread; cm_new, cm_free and cm_text from a non-real-time thread.
#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct CmInstrument CmInstrument;

uint32_t cm_abi_version(void);
uint32_t cm_num_params(void);
// plugin_dir: the plugin folder (UTF-8). params: cm_num_params() plain values, or null for defaults.
CmInstrument* cm_new(double sample_rate, const char* plugin_dir, const double* params);
void cm_free(CmInstrument* h);
void cm_set_param(const CmInstrument* h, uint32_t index, double value);
void cm_midi(const CmInstrument* h, uint8_t status, uint8_t d1, uint8_t d2);
void cm_transport(const CmInstrument* h, uint32_t playing);
// n mono samples into out
void cm_render(const CmInstrument* h, float* out, size_t n);
// kind 0: next syllable; 1: lyric set `arg` title (1-based); 2: status; 3: "position/count";
// 4: the lyrics editor's status line; 5: what is sung ("Typed", or set `arg`'s title);
// 6: the words sung before the next syllable; 7: the words after it; 8-10: the draft's three lines;
// 11: the lyrics' next line; 12: "SINGING" (0/3/6/7/11 are about the sounding syllable) or "NEXT"
size_t cm_text(const CmInstrument* h, uint32_t kind, int32_t arg, char* buf, size_t cap);
// any thread, real-time safe: an on-screen keyboard key (printable ASCII or a control code)
uint32_t cm_key(const CmInstrument* h, uint8_t code);
// changes whenever the editor's text or the typed lyrics change
uint32_t cm_text_version(const CmInstrument* h);
// any thread: changes whenever the next syllable or the words around it change (kinds 0, 3, 6, 7)
uint32_t cm_display_version(const CmInstrument* h);
// any thread, real-time safe: the head drawing's mouth pose for what is sounding (0 rest, 1 a, 2 e, 3 i, 4 o, 5 u)
uint32_t cm_pose(const CmInstrument* h);
// changes on deliberate lyric jumps only (Done, restart, source, restore, transport start)
uint32_t cm_jump_version(const CmInstrument* h);
// 1.3.3, the moving head: on/off (any thread), the pose to show (0 rest .. cm_anim_frames()-1) and a
// counter of its changes (any thread, real-time safe), the number of poses per body
void cm_set_animation(const CmInstrument* h, uint32_t on);
uint32_t cm_anim_frame(const CmInstrument* h);
uint32_t cm_anim_version(const CmInstrument* h);
uint32_t cm_anim_frames(void);
// non-real-time: the lyrics editor's state (sung text, draft, cursor) into buf if cap is enough;
// returns its length (call with cap 0 to size the buffer)
size_t cm_state_get(const CmInstrument* h, uint8_t* buf, size_t cap);
// non-real-time: the editor state from a saved project; cm_text_version follows at once
void cm_state_set(const CmInstrument* h, const uint8_t* data, size_t len);

// editor control codes (printable ASCII inserts itself)
enum { CM_K_SHIFT = 1, CM_K_BACK, CM_K_CLEAR, CM_K_ENTER, CM_K_LEFT, CM_K_RIGHT, CM_K_LOAD, CM_K_REVERT, CM_K_DONE };

#define CM_ABI_VERSION 7

#ifdef __cplusplus
}
#endif
