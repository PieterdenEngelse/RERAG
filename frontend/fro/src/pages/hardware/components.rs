use crate::pages::hardware::constants::INFO_ICON_SVG_CLASS;
use dioxus::prelude::*;

/// A small info icon (circled "i") used as a help button.
/// Matches the header info button styling with white color.
#[component]
pub fn InfoIcon() -> Element {
    rsx! {
        svg {
            class: INFO_ICON_SVG_CLASS,
            view_box: "0 0 20 20",
            fill: "none",
            stroke: "currentColor",
            circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
            line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
            circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum HelpBlock {
    Spacer,
    Heading(String),
    Paragraph(String),
    Bullets(Vec<String>),
    Ordered(Vec<String>),
    Code(String),
}

fn is_ordered_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_digit() {
        return false;
    }

    // Accept forms like "1." or "2)".
    let mut saw_more_digit = false;
    for ch in chars {
        if ch.is_ascii_digit() {
            saw_more_digit = true;
            continue;
        }
        if ch == '.' || ch == ')' {
            return true;
        }
        return false;
    }

    saw_more_digit
}

fn strip_ordered_prefix(line: &str) -> String {
    let trimmed = line.trim_start();
    let mut idx = 0;
    for (i, ch) in trimmed.char_indices() {
        if ch.is_ascii_digit() {
            idx = i + ch.len_utf8();
            continue;
        }
        if ch == '.' || ch == ')' {
            idx = i + ch.len_utf8();
            break;
        }
        break;
    }
    trimmed[idx..].trim_start().to_string()
}

fn should_render_as_code(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }

    // Simple heuristics for formulas / code-ish snippets.
    // - Contains typical operators and is relatively short
    // - Or starts with common code keywords
    let looks_like_formula =
        (t.contains('=') || t.contains("→") || t.contains('·')) && t.len() <= 80;
    let looks_like_code = t.starts_with("use ")
        || t.starts_with("import ")
        || t.starts_with("let ")
        || t.starts_with("fn ")
        || t.starts_with("A =")
        || t.starts_with("B =")
        || t.starts_with("C =")
        || t.contains("::")
        || t.contains("->");

    looks_like_formula || looks_like_code
}

fn looks_like_heading(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }

    // Headings in our help content tend to be short and not end with punctuation.
    if t.len() > 80 {
        return false;
    }

    if t.starts_with('•') || is_ordered_line(t) {
        return false;
    }

    let ends_like_sentence = t.ends_with('.') || t.ends_with(':');
    !ends_like_sentence
}

fn parse_help_blocks(paragraphs: &[&str]) -> Vec<HelpBlock> {
    let mut blocks = Vec::new();
    let mut pending_bullets: Vec<String> = Vec::new();
    let mut pending_ordered: Vec<String> = Vec::new();

    let flush_bullets = |blocks: &mut Vec<HelpBlock>, pending: &mut Vec<String>| {
        if !pending.is_empty() {
            blocks.push(HelpBlock::Bullets(std::mem::take(pending)));
        }
    };
    let flush_ordered = |blocks: &mut Vec<HelpBlock>, pending: &mut Vec<String>| {
        if !pending.is_empty() {
            blocks.push(HelpBlock::Ordered(std::mem::take(pending)));
        }
    };

    for raw in paragraphs {
        let line = raw.trim_end();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            flush_bullets(&mut blocks, &mut pending_bullets);
            flush_ordered(&mut blocks, &mut pending_ordered);
            blocks.push(HelpBlock::Spacer);
            continue;
        }

        if trimmed.starts_with('•') {
            flush_ordered(&mut blocks, &mut pending_ordered);
            pending_bullets.push(trimmed.trim_start_matches('•').trim_start().to_string());
            continue;
        }

        if is_ordered_line(trimmed) {
            flush_bullets(&mut blocks, &mut pending_bullets);
            pending_ordered.push(strip_ordered_prefix(trimmed));
            continue;
        }

        flush_bullets(&mut blocks, &mut pending_bullets);
        flush_ordered(&mut blocks, &mut pending_ordered);

        if should_render_as_code(trimmed) {
            blocks.push(HelpBlock::Code(trimmed.to_string()));
        } else if looks_like_heading(trimmed) {
            blocks.push(HelpBlock::Heading(trimmed.to_string()));
        } else {
            blocks.push(HelpBlock::Paragraph(trimmed.to_string()));
        }
    }

    flush_bullets(&mut blocks, &mut pending_bullets);
    flush_ordered(&mut blocks, &mut pending_ordered);

    blocks
}

/// Renders a modal dialog with a title and multiple paragraphs of help text.
/// Clicking outside the modal or the close button dismisses it.
///
/// The `paragraphs` slice supports simple structure using plain text markers:
/// - Empty string "" = spacer
/// - Lines starting with "•" = bullet list
/// - Lines starting with "1." / "2." = ordered list
/// - Short formula/code-ish lines (e.g. "θ(p) = p · ω") render in a monospace block
pub fn info_modal(title: &str, toggle: Signal<bool>, paragraphs: Vec<&str>) -> Element {
    let mut toggle = toggle;
    let blocks = parse_help_blocks(&paragraphs);

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| toggle.set(false),
            div {
                class: "bg-gray-900 border border-gray-700 rounded-lg p-6 w-[90vw] max-w-[90vw] max-h-[95vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),

                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-xl font-bold text-gray-100", "{title}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| toggle.set(false),
                        "×"
                    }
                }

                div { class: "text-sm text-gray-300 leading-relaxed",
                    for block in blocks {
                        match block {
                            HelpBlock::Spacer => rsx! { div { class: "h-2" } },
                            HelpBlock::Heading(text) => rsx! {
                                h4 { class: "text-sm font-semibold text-green-300 pt-2", "{text}" }
                            },
                            HelpBlock::Paragraph(text) => rsx! {
                                p { class: "mt-2 text-gray-200", "{text}" }
                            },
                            HelpBlock::Bullets(items) => rsx! {
                                ul { class: "list-disc ml-6 mt-2 space-y-1 text-gray-200",
                                    for item in items {
                                        li { class: "marker:text-primary", "{item}" }
                                    }
                                }
                            },
                            HelpBlock::Ordered(items) => rsx! {
                                ol { class: "list-decimal ml-6 mt-2 space-y-1 text-gray-200",
                                    for item in items {
                                        li { class: "marker:text-primary", "{item}" }
                                    }
                                }
                            },
                            HelpBlock::Code(text) => rsx! {
                                pre { class: "bg-gray-950 border border-gray-700 rounded p-3 mt-2 text-xs font-mono text-green-200 overflow-x-auto whitespace-pre-wrap", "{text}" }
                            },
                        }
                    }
                }
            }
        }
    }
}
