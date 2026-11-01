// See shim.h, "Vocabulary rescoring". A take is one clip recognized the way
// parakeet_capi_transcribe_pcm_batch_json_lang recognizes a batch of one,
// with the encoder projection and the greedy tokens kept, so a span of the
// transcript can be scored against another token sequence over the same
// frames from the same decoder state.
#include "shim.h"

#include "encoder.hpp"
#include "joint.hpp"
#include "mel.hpp"
#include "mel_gpu.hpp"
#include "model.hpp"
#include "prediction.hpp"
#include "backend.hpp"
#include "ggml_graph.hpp"
#include "subsampling.hpp"
#include "tokenizer.hpp"
#include "transcription.hpp"
#include "transcription_json.hpp"
#include "transducer_batch.hpp"

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <memory>
#include <string>
#include <vector>

// The C API's context as parakeet_capi.cpp of the pinned release defines it;
// the shim reads the loaded model and records errors through it.
struct parakeet_ctx {
    std::unique_ptr<pk::Model> model;
    std::string last_error;
};

namespace {

// One decoder position: the prediction-net output the joint reads, and the
// LSTM state after the inputs that produced it.
struct DecoderStep {
    std::vector<float> g;
    pk::PredState state;
};

}  // namespace

struct parakeet_sys_take {
    const pk::ModelLoader* loader = nullptr;
    int frames = 0;
    std::vector<float> enc;       // [frames, d_model]
    int d_model = 0;
    std::vector<float> enc_proj;  // [frames, joint_hidden], on the first score
    std::vector<pk::TokenInfo> tokens;
    std::vector<parakeet_sys_token> exported;
    // steps[j]: the decoder after reading SOS and the first j greedy tokens.
    std::vector<DecoderStep> steps;
};

namespace {

// Mirrors subsampling_tile_for in model.cpp (static there): the subsampling
// stage is tiled for audio long enough to overflow ggml's element limit.
int subsampling_tile(const pk::ParakeetConfig& cfg, const pk::ModelLoader& ml, int t_max) {
    if (const char* e = std::getenv("PARAKEET_SUBSAMPLING_TILE")) {
        const int t = std::atoi(e);
        if (t > 0) return t;
    }
    const long long per_t = (long long)((int)cfg.n_mels / 2) * (int)cfg.subsampling_conv_channels;
    long long win = 1 << 30;
    if (per_t > 0) {
        win = (2 * 1500000000LL) / per_t;
        win = std::max(win, 16384LL);
        win = std::min(win, 1LL << 30);
    }
    if (t_max > win) return pk::Subsampling(ml).subsample_len((int)win);
    return 0;
}

const DecoderStep& step_at(parakeet_sys_take& take, const pk::PredictionNet& pred, size_t j) {
    const int blank = (int)take.loader->config().blank_id;
    while (take.steps.size() <= j) {
        const size_t have = take.steps.size();
        DecoderStep next;
        if (have == 0) {
            pred.step(blank, true, pred.zero_state(), next.g, next.state);
        } else {
            pred.step(take.tokens[have - 1].id, false, take.steps[have - 1].state, next.g,
                      next.state);
        }
        take.steps.push_back(std::move(next));
    }
    return take.steps[j];
}

float log_add(float a, float b) {
    if (a == -std::numeric_limits<float>::infinity()) return b;
    if (b == -std::numeric_limits<float>::infinity()) return a;
    const float hi = std::max(a, b);
    return hi + std::log1p(std::exp(std::min(a, b) - hi));
}

void log_softmax(const float* in, int n, std::vector<float>& out) {
    float mx = in[0];
    for (int i = 1; i < n; ++i) mx = std::max(mx, in[i]);
    double sum = 0.0;
    for (int i = 0; i < n; ++i) sum += std::exp((double)in[i] - mx);
    const float norm = mx + (float)std::log(sum);
    out.resize(n);
    for (int i = 0; i < n; ++i) out[i] = in[i] - norm;
}

}  // namespace

extern "C" parakeet_sys_take* parakeet_sys_take_open(parakeet_ctx* ctx, const float* samples,
                                                     int n_samples, const char* target_lang,
                                                     char** json_out) {
    if (!ctx) return nullptr;
    if (!ctx->model) {
        ctx->last_error = "context has no loaded model";
        return nullptr;
    }
    if (!samples || n_samples < 0 || !json_out) {
        ctx->last_error = "invalid take arguments";
        return nullptr;
    }
    *json_out = nullptr;
    try {
        const pk::ModelLoader& loader = ctx->model->loader();
        const pk::ParakeetConfig& cfg = loader.config();
        if (cfg.tdt_durations.empty() || cfg.prompt.present) {
            ctx->last_error = "vocabulary rescoring needs a TDT model without prompt conditioning";
            return nullptr;
        }
        (void)target_lang;  // TDT models detect the language themselves.
        std::vector<float> pcm(samples, samples + n_samples);

        // Model::transcribe_16k_batch_with_timestamps for B = 1.
        pk::MelBatch mb;
        mb.B = 1;
        int n_mels = 0, t_mel = 0;
        if (std::string(pk::global_backend().device_name()) != "cpu") {
            pk::GpuMel(loader).compute(pcm, mb.data, n_mels, t_mel);
        } else {
            pk::MelFrontend(loader).compute(pcm, mb.data, n_mels, t_mel);
        }
        mb.n_mels = n_mels;
        mb.T_max = t_mel;
        mb.valid_T = {t_mel};
        pk::Encoder encoder(loader);
        std::vector<std::vector<float>> enc_outs;
        std::vector<int> valid;
        int d_model = 0, t_out = 0;
        const int tile = subsampling_tile(cfg, loader, mb.T_max);
        if (tile > 0) {
            encoder.forward_batch_tiled(mb, enc_outs, d_model, t_out, valid, tile);
        } else {
            encoder.forward_batch(mb, enc_outs, d_model, t_out, valid);
        }
        const int frames = valid[0];
        std::vector<std::vector<float>> encs(1);
        encs[0].resize((size_t)frames * d_model);
        for (int t = 0; t < frames; ++t)
            for (int c = 0; c < d_model; ++c)
                encs[0][(size_t)t * d_model + c] = enc_outs[0][(size_t)c * frames + t];
        pk::PredictionNet pred(loader);
        pk::Joint joint(loader);
        std::vector<std::vector<int32_t>> ids;
        std::vector<std::vector<pk::TokenInfo>> toks;
        pk::transducer_greedy_batch(pred, joint, encs, {frames}, d_model, cfg.tdt_durations,
                                    (int)cfg.blank_id, (int)cfg.max_symbols, ids, &toks);
        const float frame_sec =
            (float)cfg.hop_length * (float)cfg.subsampling_factor / (float)cfg.sample_rate;
        pk::Transcription tr;
        tr.text = pk::detokenize(loader.tokenizer_pieces(),
                                 pk::strip_special_tokens(loader.tokenizer_pieces(), ids[0]));
        tr.words = pk::group_words(toks[0], loader.tokenizer_pieces(), frame_sec);
        tr.tokens = toks[0];
        const std::string json = pk::transcription_to_json(tr, frame_sec);

        auto take = std::make_unique<parakeet_sys_take>();
        take->loader = &loader;
        take->frames = frames;
        take->enc = std::move(encs[0]);
        take->d_model = d_model;
        take->tokens = std::move(toks[0]);
        for (const pk::TokenInfo& t : take->tokens)
            take->exported.push_back(parakeet_sys_token{t.id, t.frame, t.span});

        char* out = (char*)std::malloc(json.size() + 1);
        if (!out) {
            ctx->last_error = "out of memory";
            return nullptr;
        }
        std::memcpy(out, json.c_str(), json.size() + 1);
        *json_out = out;
        ctx->last_error.clear();
        return take.release();
    } catch (const std::exception& e) {
        ctx->last_error = e.what();
    } catch (...) {
        ctx->last_error = "unknown error";
    }
    return nullptr;
}

extern "C" int parakeet_sys_take_tokens(const parakeet_sys_take* take,
                                        const parakeet_sys_token** out) {
    *out = take->exported.data();
    return (int)take->exported.size();
}

extern "C" int parakeet_sys_take_frames(const parakeet_sys_take* take) { return take->frames; }

extern "C" int parakeet_sys_take_score(parakeet_sys_take* take, int start, int frame_lo,
                                       int frame_hi, const int32_t* tokens, int n_tokens,
                                       float* out) {
    if (!take || !out || start < 0 || (size_t)start > take->tokens.size() || frame_lo < 0 ||
        frame_hi > take->frames || frame_lo >= frame_hi || n_tokens < 0 ||
        (n_tokens > 0 && !tokens))
        return 1;
    try {
        const pk::ModelLoader& loader = *take->loader;
        const pk::ParakeetConfig& cfg = loader.config();
        pk::PredictionNet pred(loader);
        pk::Joint joint(loader);
        const int vocab = (int)cfg.vocab_size;
        for (int i = 0; i < n_tokens; ++i)
            if (tokens[i] < 0 || tokens[i] >= vocab) return 1;

        // The decoder outputs along the forced path: g[0] where greedy
        // decoding stood before token `start`, g[u] after reading the
        // path's first u tokens.
        const int P = pred.hidden_size();
        std::vector<float> g((size_t)(n_tokens + 1) * P);
        const DecoderStep& at = step_at(*take, pred, (size_t)start);
        std::copy(at.g.begin(), at.g.end(), g.begin());
        pk::PredState state = at.state;
        for (int u = 0; u < n_tokens; ++u) {
            std::vector<float> next_g;
            pk::PredState next_state;
            pred.step(tokens[u], false, state, next_g, next_state);
            std::copy(next_g.begin(), next_g.end(), g.begin() + (size_t)(u + 1) * P);
            state = std::move(next_state);
        }

        // A take nothing is scored on never pays for the projection.
        if (take->enc_proj.empty())
            joint.precompute_enc_proj(take->enc, take->frames, take->d_model, take->enc_proj);

        // Joint logits for every (frame, position) of the window, batched.
        const int F = frame_hi - frame_lo;
        const int U1 = n_tokens + 1;
        const int H = joint.joint_hidden();
        const int V = joint.V_plus();
        const int n = F * U1;
        std::vector<float> ep((size_t)n * H), gp((size_t)n * P), logits;
        for (int f = 0; f < F; ++f)
            for (int u = 0; u < U1; ++u) {
                const int k = f * U1 + u;
                std::copy_n(take->enc_proj.data() + (size_t)(frame_lo + f) * H, H,
                            ep.data() + (size_t)k * H);
                std::copy_n(g.data() + (size_t)u * P, P, gp.data() + (size_t)k * P);
            }
        joint.step_logits_batch(ep.data(), gp.data(), P, n, logits);

        // Forward algorithm over the TDT lattice: token and duration slices
        // log-softmaxed apart as NeMo's TDT loss does; a blank never takes a
        // zero duration; the path ends with every token read at frame_hi (a
        // jump past it lands there).
        const std::vector<int32_t>& durations = cfg.tdt_durations;
        const int D = (int)durations.size();
        const int token_count = V - D;
        const int blank = (int)cfg.blank_id;
        const float NEG = -std::numeric_limits<float>::infinity();
        std::vector<float> alpha((size_t)(F + 1) * U1, NEG);
        alpha[0] = 0.0f;
        float final = NEG;
        std::vector<float> tok_lp, dur_lp;
        for (int f = 0; f < F; ++f)
            for (int u = 0; u < U1; ++u) {
                const float a = alpha[(size_t)f * U1 + u];
                if (a == NEG) continue;
                const float* row = logits.data() + (size_t)(f * U1 + u) * V;
                log_softmax(row, token_count, tok_lp);
                log_softmax(row + token_count, D, dur_lp);
                for (int d = 0; d < D; ++d) {
                    const int to = f + durations[d];
                    if (u < n_tokens) {
                        const float lp = a + tok_lp[tokens[u]] + dur_lp[d];
                        if (to >= F) {
                            if (u + 1 == n_tokens) final = log_add(final, lp);
                        } else {
                            float& cell = alpha[(size_t)to * U1 + u + 1];
                            cell = log_add(cell, lp);
                        }
                    }
                    if (durations[d] > 0) {
                        const float lp = a + tok_lp[blank] + dur_lp[d];
                        if (to >= F) {
                            if (u == n_tokens) final = log_add(final, lp);
                        } else {
                            float& cell = alpha[(size_t)to * U1 + u];
                            cell = log_add(cell, lp);
                        }
                    }
                }
            }
        *out = final;
        return 0;
    } catch (...) {
        return 1;
    }
}

extern "C" void parakeet_sys_take_free(parakeet_sys_take* take) { delete take; }

extern "C" int parakeet_sys_vocab_size(parakeet_ctx* ctx) {
    if (!ctx || !ctx->model) return 0;
    return (int)ctx->model->loader().tokenizer_pieces().size();
}

extern "C" const char* parakeet_sys_piece(parakeet_ctx* ctx, int id) {
    if (!ctx || !ctx->model) return nullptr;
    const std::vector<std::string>& pieces = ctx->model->loader().tokenizer_pieces();
    if (id < 0 || (size_t)id >= pieces.size()) return nullptr;
    return pieces[(size_t)id].c_str();
}
