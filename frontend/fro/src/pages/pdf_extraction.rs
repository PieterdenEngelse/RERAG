//! Relational PDF extraction view — visualises the per-page line / column
//! split produced by the Phase 1 column-aware native PDF extractor.
//!
//! Educational mission: surface what the extractor saw — left/right columns,
//! silhouette confidence per page, individual line text. The default for a
//! visitor with no input is the demo fixture `two_column_invoice.pdf`, so
//! the page is "useful" even when arrived at directly from the nav.

use crate::api::{self, PdfExtractionResponse, PdfLineRow, PdfPageRow};
use crate::app::Route;
use crate::components::monitor::nav_tabs::NavTabs;
use crate::pages::hardware::components::{info_modal, InfoIcon};
use crate::pages::hardware::constants::{PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE};
use dioxus::prelude::*;

#[component]
pub fn PdfExtraction() -> Element {
    let mut document_id = use_signal(|| "two_column_invoice.pdf".to_string());
    let mut data: Signal<Option<PdfExtractionResponse>> = use_signal(|| None);
    let mut error: Signal<Option<String>> = use_signal(|| None);
    let mut loading = use_signal(|| false);
    let mut show_info = use_signal(|| false);
    let show_silhouette_info = use_signal(|| false);

    let load = move |doc_id: String| {
        spawn(async move {
            loading.set(true);
            error.set(None);
            match api::fetch_pdf_extraction(&doc_id).await {
                Ok(resp) => {
                    data.set(Some(resp));
                }
                Err(e) => {
                    error.set(Some(e));
                    data.set(None);
                }
            }
            loading.set(false);
        });
    };

    // Initial load
    use_effect(move || {
        let id = document_id.read().clone();
        load(id);
    });

    rsx! {
        div {
            class: "min-h-screen bg-base-200 p-6 text-gray-200",
            div {
                class: "max-w-6xl mx-auto",

                NavTabs { active: Route::PdfExtraction {} }

                // Header + What-is-this info button.
                div {
                    class: "flex items-center gap-2 mt-4 mb-3",
                    h1 {
                        class: "text-2xl font-bold",
                        "PDF Extraction — Relational view"
                    }
                    button {
                        class: PARAM_ICON_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        title: "What this view is teaching",
                        onclick: move |_| show_info.set(true),
                        InfoIcon {}
                    }
                }
                p {
                    class: "text-gray-300 mb-6 leading-relaxed",
                    "Every line on every page, grouped by the column the relational extractor placed it in. \
                     Adaptive k (2..=6) picks the best column count per page; the silhouette score is the \
                     detector's confidence in that pick. Low scores (yellow badge) mean no k cleared the \
                     threshold and the page is tagged Multi."
                }

                // Document id input
                div {
                    class: "flex gap-2 mb-6 items-center",
                    label {
                        class: "text-gray-400",
                        "Document:"
                    }
                    input {
                        class: "input input-sm input-bordered bg-gray-700 text-gray-200 flex-1",
                        r#type: "text",
                        value: "{document_id}",
                        oninput: move |e| document_id.set(e.value()),
                    }
                    button {
                        class: "btn btn-sm",
                        style: "background-color:#7C2A02;border:1px solid #7C2A02;color:#fff;",
                        onclick: move |_| {
                            let id = document_id.read().clone();
                            load(id);
                        },
                        "Load"
                    }
                }

                if *loading.read() {
                    div { class: "text-gray-400", "Loading…" }
                }

                if let Some(err) = error.read().clone() {
                    div {
                        class: "alert alert-error mb-4",
                        "Error: {err}"
                    }
                }

                if let Some(resp) = data.read().clone() {
                    SummaryHeader { resp: resp.clone(), on_silhouette_info: show_silhouette_info }
                    PageList { resp: resp }
                }
            }
        }

        if *show_info.read() {
            { info_modal(
                "Relational PDF Extraction (Phase 1)",
                show_info,
                vec![
                    "A naive PDF reader concatenates all text on a page into one long string, destroying column relationships. On a two-column invoice — \"Renewal fee\" on the left, \"EUR 200\" on the right — labels and values get mixed and the LLM can't tell which number pairs with which label.",
                    "Relational extraction first y-clusters words into lines (with an x-gap split so reading-order PDFs don't merge across columns), then runs adaptive-k k-means on each line's horizontal position (k ∈ 2..=6, chosen by silhouette). Every line gets tagged Col(0), Col(1), …, Single, or Multi. The chunker then treats any same-page transition between different columns as a strong boundary, so chunks stay column-pure. The \"renewal fee\" question retrieves a chunk with only right-column content.",
                    "This view shows what the extractor saw: which lines landed in which column on which page, and the silhouette score for the chosen k.",
                ],
            ) }
        }

        if *show_silhouette_info.read() {
            { info_modal(
                "Silhouette score",
                show_silhouette_info,
                vec![
                    "Silhouette (–1 to +1) measures how clean a k-means split is. Higher = clusters are well-separated. Adaptive-k picks the highest-silhouette k in 2..=6; if no k clears ≈0.30, every line on the page is tagged Multi (we refuse to guess).",
                    "The k that was actually used and its silhouette are persisted per page so future tuning has historical data to calibrate against without a schema change.",
                ],
            ) }
        }
    }
}

#[component]
fn SummaryHeader(resp: PdfExtractionResponse, on_silhouette_info: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "grid grid-cols-3 gap-4 mb-6",
            div {
                class: "bg-gray-800 rounded p-4",
                div { class: "text-xs text-gray-400 uppercase", "Pages" }
                div { class: "text-2xl font-bold text-gray-100", "{resp.page_count}" }
            }
            div {
                class: "bg-gray-800 rounded p-4",
                div { class: "text-xs text-gray-400 uppercase", "Lines" }
                div { class: "text-2xl font-bold text-gray-100", "{resp.line_count}" }
            }
            div {
                class: "bg-gray-800 rounded p-4 flex items-center gap-2",
                div {
                    class: "flex-1",
                    div { class: "text-xs text-gray-400 uppercase", "Avg silhouette" }
                    div { class: "text-2xl font-bold text-gray-100", "{avg_silhouette(&resp.pages)}" }
                }
                button {
                    class: PARAM_ICON_BUTTON_CLASS,
                    style: PARAM_ICON_BUTTON_STYLE,
                    title: "What silhouette means",
                    onclick: move |_| on_silhouette_info.set(true),
                    InfoIcon {}
                }
            }
        }
    }
}

#[component]
fn PageList(resp: PdfExtractionResponse) -> Element {
    rsx! {
        div {
            class: "space-y-6",
            for page in resp.pages.iter().cloned() {
                PagePanel {
                    key: "{page.page}",
                    page: page.clone(),
                    lines: resp
                        .lines
                        .iter()
                        .filter(|l| l.page == page.page)
                        .cloned()
                        .collect::<Vec<_>>(),
                }
            }
            if resp.pages.is_empty() {
                div {
                    class: "text-gray-300 text-sm",
                    "No relational extraction data found for this document. \
                     Make sure the corpus has PDF_RELATIONAL_ENABLED on, and re-upload the PDF."
                }
            }
        }
    }
}

#[component]
fn PagePanel(page: PdfPageRow, lines: Vec<PdfLineRow>) -> Element {
    let silhouette_str = match page.column_silhouette {
        Some(s) => format!("{:.2}", s),
        None => "—".to_string(),
    };
    let silhouette_class = silhouette_badge_class(page.column_silhouette);

    rsx! {
        div {
            class: "bg-gray-800 rounded p-4",
            div {
                class: "flex items-center gap-3 mb-3",
                div { class: "text-lg font-semibold text-gray-100", "Page {page.page}" }
                div { class: "text-xs text-gray-400", "{page.line_count} lines" }
                div { class: "text-xs text-gray-400", "k={page.column_k_used}" }
                div { class: silhouette_class, "silhouette {silhouette_str}" }
                if page.is_scanned {
                    div {
                        class: "badge badge-sm bg-yellow-900 text-yellow-200 border-yellow-800",
                        "scanned"
                    }
                }
            }
            LinesCanvas { lines: lines.clone() }
            div {
                class: "space-y-1 text-sm",
                for line in lines {
                    LineRowView { key: "{line.page}-{line.line_idx}", line: line }
                }
            }
        }
    }
}

/// SVG canvas of every line bbox on a page, coloured by column. Coordinates
/// are already normalised to 0..1000 in both axes (see
/// `backend/src/pdf/word_extractor.rs`) with y flipped to screen orientation,
/// so no transform is required. Real pages aren't square; rendering into a
/// square viewBox is intentional — the user sees how the lines cluster
/// horizontally (columns) and where they sit on the page.
#[component]
fn LinesCanvas(lines: Vec<PdfLineRow>) -> Element {
    let boxes: Vec<LineBox> = lines
        .iter()
        .filter_map(|l| match (l.x0, l.y0, l.x1, l.y1) {
            (Some(x0), Some(y0), Some(x1), Some(y1)) => Some(LineBox {
                key: format!("{}-{}", l.page, l.line_idx),
                x: x0,
                y: y0,
                w: (x1 - x0).max(1),
                h: (y1 - y0).max(1),
                fill: column_fill(&l.column_position),
            }),
            _ => None,
        })
        .collect();

    if boxes.is_empty() {
        return rsx! {
            div {
                class: "text-gray-400 text-xs mb-3 italic",
                "No bounding-box data on this page — likely the extractous text-only fallback."
            }
        };
    }

    rsx! {
        div {
            class: "flex justify-center mb-3",
            svg {
                width: "320",
                height: "320",
                view_box: "0 0 1000 1000",
                preserve_aspect_ratio: "xMidYMid meet",
                style: "background-color:#111827;border:1px solid #374151;border-radius:0.25rem;",
                for b in boxes {
                    rect {
                        key: "{b.key}",
                        x: "{b.x}",
                        y: "{b.y}",
                        width: "{b.w}",
                        height: "{b.h}",
                        fill: "{b.fill}",
                        opacity: "0.7",
                    }
                }
            }
        }
    }
}

struct LineBox {
    key: String,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    fill: &'static str,
}

/// Hex equivalents of the badge palette in `column_badge`, brighter so they
/// pop on the canvas's dark backdrop.
fn column_fill(column: &str) -> &'static str {
    if column == "single" {
        return "#9ca3af"; // gray-400
    }
    if column == "multi" {
        return "#facc15"; // yellow-400
    }
    if let Some(n) = column.strip_prefix("col").and_then(|n| n.parse::<u8>().ok()) {
        const PALETTE: &[&str] = &[
            "#3b82f6", // blue-500
            "#a855f7", // purple-500
            "#10b981", // emerald-500
            "#f59e0b", // amber-500
            "#ec4899", // pink-500
            "#06b6d4", // cyan-500
        ];
        return PALETTE.get(n as usize).copied().unwrap_or("#9ca3af");
    }
    "#9ca3af"
}

#[component]
fn LineRowView(line: PdfLineRow) -> Element {
    let (badge_class, badge_label) = column_badge(&line.column_position);
    rsx! {
        div {
            class: "flex items-start gap-2 leading-snug",
            div { class: badge_class, "{badge_label}" }
            div { class: "flex-1 text-gray-200", "{line.text}" }
        }
    }
}

/// Wire format from the backend: 'single' | 'multi' | 'col<n>' (0-based,
/// left-to-right). The badge palette is indexed by `n` and 1-indexed in the
/// label so users see "C1, C2, C3" instead of "C0, C1, C2".
fn column_badge(column: &str) -> (String, String) {
    let base = "px-2 py-0.5 rounded text-xs border w-16 text-center shrink-0";
    if column == "single" {
        return (
            format!("{base} bg-gray-700 text-gray-300 border-gray-600"),
            "S".into(),
        );
    }
    if column == "multi" {
        return (
            format!("{base} bg-yellow-900 text-yellow-200 border-yellow-800"),
            "M".into(),
        );
    }
    if let Some(n) = column.strip_prefix("col").and_then(|n| n.parse::<u8>().ok()) {
        const PALETTE: &[&str] = &[
            "bg-blue-900 text-blue-200 border-blue-800",
            "bg-purple-900 text-purple-200 border-purple-800",
            "bg-emerald-900 text-emerald-200 border-emerald-800",
            "bg-amber-900 text-amber-200 border-amber-800",
            "bg-pink-900 text-pink-200 border-pink-800",
            "bg-cyan-900 text-cyan-200 border-cyan-800",
        ];
        let color = PALETTE
            .get(n as usize)
            .copied()
            .unwrap_or("bg-gray-700 text-gray-300 border-gray-600");
        return (format!("{base} {color}"), format!("C{}", n + 1));
    }
    (
        format!("{base} bg-gray-700 text-gray-300 border-gray-600"),
        "?".into(),
    )
}

fn silhouette_badge_class(s: Option<f32>) -> &'static str {
    match s {
        Some(v) if v >= 0.30 => {
            "badge badge-sm bg-green-900 text-green-200 border-green-800"
        }
        Some(_) => "badge badge-sm bg-yellow-900 text-yellow-200 border-yellow-800",
        None => "badge badge-sm bg-gray-700 text-gray-300 border-gray-600",
    }
}

fn avg_silhouette(pages: &[PdfPageRow]) -> String {
    let scored: Vec<f32> = pages.iter().filter_map(|p| p.column_silhouette).collect();
    if scored.is_empty() {
        return "—".to_string();
    }
    let avg: f32 = scored.iter().sum::<f32>() / scored.len() as f32;
    format!("{:.2}", avg)
}
