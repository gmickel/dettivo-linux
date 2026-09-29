//! The generation CPU fallback's reload: a model that the GPU could not
//! run moves to the CPU for the rest of the engine's life.

use std::sync::atomic::Ordering;

use dettivo_engine_proto::Backend;

use super::{Engine, EngineError, LAST_DEVICE, load_failed};

impl Engine {
    /// Reloads the model, and its adapter, on the CPU for good.
    pub(super) fn move_to_cpu(&mut self, why: &str) -> Result<(), EngineError> {
        let model = Self::try_load(&self.path, None)?;
        let lora = match self.lora_path.as_deref() {
            Some(adapter) => Some(model.lora_adapter_init(adapter).map_err(|e| {
                load_failed(
                    adapter,
                    format!("llama.cpp could not load the adapter ({e})"),
                )
            })?),
            None => None,
        };
        self.lora = lora;
        self.model = model;
        self.device = None;
        self.gpu_fallback = false;
        LAST_DEVICE.store(usize::MAX, Ordering::Relaxed);
        self.loaded.backend = Backend::Cpu;
        self.loaded.reason = format!(
            "{}; the GPU could not run it ({why}), CPU fallback",
            self.loaded.reason
        );
        Ok(())
    }
}
