// See shim.h. Includes parakeet.cpp's private headers from the pinned
// source tree; the build script points the compiler at them.
#include "shim.h"

#include "backend.hpp"
#include "ggml_graph.hpp"

#ifndef PARAKEET_SYS_SOURCE_VERSION
#define PARAKEET_SYS_SOURCE_VERSION "unknown"
#endif

extern "C" const char* parakeet_sys_backend_device_name(void) {
    return pk::global_backend().device_name();
}

extern "C" void parakeet_sys_set_num_threads(int n) {
    pk::set_num_threads(n);
}

extern "C" void parakeet_sys_shutdown_backend(void) {
    pk::shutdown_backend();
}

extern "C" const char* parakeet_sys_source_version(void) {
    return PARAKEET_SYS_SOURCE_VERSION;
}
