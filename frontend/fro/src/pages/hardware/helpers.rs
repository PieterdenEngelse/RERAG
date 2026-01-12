use crate::api;

/// Formats a model info struct into a human-readable label for display in dropdowns.
pub fn format_model_label(model: &api::ModelInfo) -> String {
    let mut parts = vec![model.name.clone()];
    if let Some(family) = &model.family {
        if !family.is_empty() {
            parts.push(format!("({})", family));
        }
    } else if model.is_custom {
        parts.push("(custom)".to_string());
    }
    let size = model.size_display();
    if !size.is_empty() {
        parts.push(format!("- {}", size));
    }
    if let Some(modified) = &model.modified_at {
        parts.push(format!("updated {}", modified));
    }
    if let Some(desc) = &model.description {
        if !desc.is_empty() {
            parts.push(format!("- {}", desc));
        }
    }
    if model.is_active {
        parts.push("• active".to_string());
    }
    parts.join(" ")
}

/// Formats GPU info into a human-readable label for display.
pub fn format_gpu_label(gpu: &api::GpuInfo) -> String {
    format!(
        "GPU {} · {} · {} ({})",
        gpu.index, gpu.name, gpu.vendor, gpu.device_type
    )
}
