pub const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
#[allow(dead_code)]
pub const PARAM_BLOCK_CLASS_TIGHT: &str = "flex flex-col text-xs text-gray-200";
pub const PARAM_COLUMN_CLASS: &str = "param-column-spacing";
#[allow(dead_code)]
pub const PARAM_INPUT_ROW_CLASS: &str = "flex items-end gap-2";
pub const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";
pub const PARAM_LABEL_CLASS_TIGHT: &str =
    "text-gray-400 whitespace-nowrap inline-block mb-[-1.5mm]";
pub const PARAM_ICON_BUTTON_CLASS: &str =
    "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80";
pub const PARAM_ICON_BUTTON_STYLE: &str = "background-color: #7C2A02; border: 1px solid #7C2A02;"; // Rust brand color
pub const PARAM_NUMBER_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
pub const PARAM_TEXT_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 w-72";
pub const PARAM_TEXT_INPUT_COMPACT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 w-40";

// Shared info button/icon styling
pub const INFO_ICON_SVG_CLASS: &str = "w-5 h-5 text-white";
pub const QUICK_ACTION_INFO_BUTTON_CLASS: &str = PARAM_ICON_BUTTON_CLASS;
pub const QUICK_ACTION_INFO_ICON_CLASS: &str = INFO_ICON_SVG_CLASS;
