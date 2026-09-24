/* A small C surface over parakeet.cpp internals the C API does not expose:
 * the device the process-global ggml backend runs on, its thread count and
 * its shutdown (so a failed GPU backend can be replaced by the CPU one in
 * the same process). Compiled from shim.cpp against the pinned source. */
#ifndef PARAKEET_SYS_SHIM_H
#define PARAKEET_SYS_SHIM_H

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

#ifdef __cplusplus
}
#endif

#endif
