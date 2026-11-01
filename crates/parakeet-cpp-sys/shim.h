/* A small C surface over parakeet.cpp internals the C API does not expose:
 * the device the process-global ggml backend runs on, its thread count and
 * its shutdown (so a failed GPU backend can be replaced by the CPU one in
 * the same process), and what vocabulary rescoring reads from a decode.
 * Compiled from shim.cpp and bias.cpp against the pinned source. */
#ifndef PARAKEET_SYS_SHIM_H
#define PARAKEET_SYS_SHIM_H

#include <stdint.h>

#include "parakeet_capi.h"

#ifdef __cplusplus
extern "C" {
#endif

/* The ggml device name the backend runs on ("cpu", "Vulkan0", ...). The
 * backend is created on first use; calling this creates it. The pointer is
 * owned by the backend and valid until parakeet_sys_shutdown_backend. */
const char* parakeet_sys_backend_device_name(void);

/* Sets the compute thread count for every graph (0 restores the default). */
void parakeet_sys_set_num_threads(int n);

/* Frees the process-global backend; the next inference recreates it. */
void parakeet_sys_shutdown_backend(void);

/* The pinned parakeet.cpp version this crate was built from. */
const char* parakeet_sys_source_version(void);

/* Vocabulary rescoring (bias.cpp). A take is one clip recognized exactly as
 * parakeet_capi_transcribe_pcm_batch_json_lang recognizes a batch of one,
 * with the encoder output and the greedy tokens kept, so a span of it can
 * be scored against another token sequence. TDT models without prompt
 * conditioning only. */
typedef struct parakeet_sys_take parakeet_sys_take;

/* One greedy token: its id, the encoder frame it was emitted at and the
 * frames decoding advanced by with it. */
typedef struct parakeet_sys_token {
    int32_t id;
    int32_t frame;
    int32_t span;
} parakeet_sys_token;

/* Recognizes `n_samples` 16 kHz mono floats. `*json_out` receives the
 * clip's document (the element the batch entry point's array holds), freed
 * with parakeet_capi_free_string. NULL on failure, with the reason in
 * parakeet_capi_last_error. The take is freed with parakeet_sys_take_free. */
parakeet_sys_take* parakeet_sys_take_open(parakeet_ctx* ctx, const float* samples,
                                          int n_samples, const char* target_lang,
                                          char** json_out);

/* The greedy tokens; the array lives as long as the take. */
int parakeet_sys_take_tokens(const parakeet_sys_take* take, const parakeet_sys_token** out);

/* The encoder frame count. */
int parakeet_sys_take_frames(const parakeet_sys_take* take);

/* The total log-probability (nats, summed over every TDT alignment) that the
 * decoder, standing where greedy decoding stood before greedy token `start`
 * and at frame `frame_lo`, reads exactly `tokens` and reaches `frame_hi`;
 * -inf when no alignment fits. Returns 0, or nonzero for bad arguments or a
 * failed graph. */
int parakeet_sys_take_score(parakeet_sys_take* take, int start, int frame_lo, int frame_hi,
                            const int32_t* tokens, int n_tokens, float* out);

void parakeet_sys_take_free(parakeet_sys_take* take);

/* The tokenizer's piece count, and one piece (owned by the model). */
int parakeet_sys_vocab_size(parakeet_ctx* ctx);
const char* parakeet_sys_piece(parakeet_ctx* ctx, int id);

#ifdef __cplusplus
}
#endif

#endif
