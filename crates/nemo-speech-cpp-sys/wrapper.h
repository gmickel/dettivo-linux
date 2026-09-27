/* The headers bindgen reads: NeMo-Speech.cpp's standalone diarization C API,
 * which pulls in the shared status codes and last-error accessor of asr.h,
 * and ggml's device registry, which libnemo_speech_asr.so exports, so the
 * engine can name the GPU a model loaded on. */
#include "nemo_speech/diar.h"
#include "ggml-backend.h"
