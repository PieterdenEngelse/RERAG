use crate::api::HardwareConfig;
use dioxus::prelude::*;

#[derive(Clone)]
pub struct HardwarePageState {
    pub hardware_config: Signal<HardwareConfig>,
}

impl Default for HardwarePageState {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwarePageState {
    pub fn new() -> Self {
        Self {
            hardware_config: use_signal(HardwareConfig::default),
        }
    }
}
