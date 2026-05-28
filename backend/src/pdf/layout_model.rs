// src/pdf/layout_model.rs — Stage 2: region classification.
//
// Three-tier priority:
//   1. DETR (image-based): pdfium renders each page → PubLayNet DETR ONNX detects
//      region bboxes → words assigned to regions by containment.
//      Needs: LAYOUT_DETR_MODEL_PATH + PDFium library (PDFIUM_LIBRARY_PATH or system).
//      ONNX export: see tools/export_detr.py
//
//   2. Word-ORT (text+bbox): ONNX classifier on per-word bbox features.
//      Needs: LAYOUT_ORT_MODEL_PATH
//      Contract: input "word_features" [1, n, 8] f32 → output [n] i64 labels
//        features: [x0/1000, y0/1000, x1/1000, y1/1000, w/1000, h/1000, word_len/50, page/100]
//        labels:   0=Text 1=Title 2=SectionHeader 3=Table 4=Figure 5=List 6=Footer 7=Header 8=Caption 9=Other
//
//   3. Heuristic: rule-based, always available, no downloads.
//
// DETR model contract
// ───────────────────
// Input:  "pixel_values"  [1, 3, H, W]  f32  ImageNet-normalised
// Output: "logits"        [1, Q, C+1]   f32  class logits (C classes + background)
//         "pred_boxes"    [1, Q, 4]     f32  (cx, cy, w, h) normalised 0–1
//
// cmarkea/detr-layout-detection (C=11): 0=Caption 1=Footnote 2=Formula 3=List-item 4=Page-footer
//   5=Page-header 6=Picture 7=Section-header 8=Table 9=Text 10=Title  (11=no-object)
// Model: cmarkea/detr-layout-detection  →  models/layout_detr/model.onnx
// Confidence threshold: LAYOUT_DETR_THRESHOLD (default 0.7)

use super::word_extractor::WordSpan;
use ort::{inputs, session::Session, value::Tensor};
use std::sync::OnceLock;
use tracing::{debug, info, warn};

// ── Region tag enum ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionTag {
    Text,
    Title,
    SectionHeader,
    Table,
    Figure,
    List,
    Footer,
    Header,
    Caption,
    Other,
}

impl RegionTag {
    pub fn from_docbank_label(label: &str) -> Self {
        match label {
            "title" => Self::Title,
            "section" | "abstract" => Self::SectionHeader,
            "paragraph" | "reference" | "author" => Self::Text,
            "table" => Self::Table,
            "figure" => Self::Figure,
            "list" => Self::List,
            "footer" => Self::Footer,
            "caption" => Self::Caption,
            "equation" => Self::Other,
            _ => Self::Text,
        }
    }
}

// ── LayoutModel ───────────────────────────────────────────────────────────────

pub struct LayoutModel {
    inner: LayoutModelInner,
    /// Human-readable description of which tier loaded and from where.
    /// Surfaced to the UI on /config/onnx so the user can tell Tier 0 (HF Hub)
    /// from Tier 1 (local DETR) from Tier 2 (word-feature ORT) at a glance.
    source_label: String,
}

enum LayoutModelInner {
    Detr(std::sync::Mutex<DetrLayoutModel>),
    WordOrt(std::sync::Mutex<OrtLayoutClassifier>),
    Heuristic,
}

static MODEL: OnceLock<LayoutModel> = OnceLock::new();

/// Resolve an `owner/repo[:filename]` spec to a local cached path. Triggers a
/// one-time HuggingFace Hub download into `~/.cache/huggingface/hub/` if the
/// file isn't already cached. Default filename when none is specified:
/// `model.onnx` (matches what the DETR / word-ORT loaders below expect via
/// `Session::commit_from_file`).
///
/// Returns `Ok(path)` on success; `Err(...)` on network failure, missing repo,
/// or missing file — the caller is expected to log a warning and fall through.
fn resolve_hf_model(spec: &str) -> anyhow::Result<std::path::PathBuf> {
    let (repo_id, filename) = match spec.split_once(':') {
        Some((r, f)) => (r.to_string(), f.to_string()),
        None => (spec.to_string(), "model.onnx".to_string()),
    };

    info!(
        repo = %repo_id,
        file = %filename,
        "Resolving HF Hub model (will download to ~/.cache/huggingface/hub/ if not cached)"
    );

    let api = hf_hub::api::sync::Api::new()
        .map_err(|e| anyhow::anyhow!("hf-hub Api::new failed: {e}"))?;
    let repo = api.model(repo_id.clone());
    let path = repo
        .get(&filename)
        .map_err(|e| anyhow::anyhow!("hf-hub download {repo_id}/{filename} failed: {e}"))?;

    info!(
        repo = %repo_id,
        file = %filename,
        local_path = %path.display(),
        "HF Hub model ready"
    );
    Ok(path)
}

impl LayoutModel {
    pub fn load_or_heuristic() -> &'static LayoutModel {
        MODEL.get_or_init(|| {
            // Tier 0: HuggingFace Hub auto-download via LAYOUT_ML_MODEL_ID.
            // Highest priority because it's the "just works" UX path. Spec
            // is `owner/repo` (defaults to `model.onnx`) or
            // `owner/repo:custom-filename.onnx` if the model file is named
            // something else inside the repo. Resolved path is fed into the
            // ORT session via the DETR loader — matches the format the rest
            // of the pipeline expects.
            let hf_spec = crate::settings::effective_or("LAYOUT_ML_MODEL_ID", "");
            if !hf_spec.is_empty() {
                match resolve_hf_model(&hf_spec) {
                    Ok(local_path) => {
                        // Build a DETR-style session from the downloaded file.
                        // Threshold and num_classes are read from their own
                        // env vars below (or sensible defaults).
                        let path_str = local_path.display().to_string();
                        match Session::builder()
                            .map_err(|e| anyhow::anyhow!("{e}"))
                            .and_then(|mut b| {
                                b.commit_from_file(&local_path)
                                    .map_err(|e| anyhow::anyhow!("{e}"))
                            }) {
                            Ok(session) => {
                                let threshold =
                                    crate::settings::effective_f64("LAYOUT_DETR_THRESHOLD", 0.7)
                                        as f32;
                                let num_classes = crate::settings::effective_u64(
                                    "LAYOUT_DETR_NUM_CLASSES",
                                    11,
                                ) as usize;
                                info!(
                                    model = %path_str,
                                    "Layout model loaded from HF Hub (via LAYOUT_ML_MODEL_ID)"
                                );
                                return LayoutModel {
                                    inner: LayoutModelInner::Detr(std::sync::Mutex::new(
                                        DetrLayoutModel {
                                            session,
                                            threshold,
                                            num_classes,
                                        },
                                    )),
                                    source_label: format!("DETR (HF Hub: {hf_spec})"),
                                };
                            }
                            Err(e) => warn!(
                                error = %e,
                                "HF Hub model downloaded but failed to load — falling through"
                            ),
                        }
                    }
                    Err(e) => warn!(
                        error = %e,
                        "LAYOUT_ML_MODEL_ID set but download failed — falling through to local-path tiers"
                    ),
                }
            }

            // Tier 1: DETR (local path)
            if !crate::settings::effective_or("LAYOUT_DETR_MODEL_PATH", "").is_empty() {
                match DetrLayoutModel::load() {
                    Ok(m) => {
                        let path = crate::settings::effective_or("LAYOUT_DETR_MODEL_PATH", "");
                        info!("DETR layout model loaded (image-based PubLayNet)");
                        return LayoutModel {
                            inner: LayoutModelInner::Detr(std::sync::Mutex::new(m)),
                            source_label: format!("DETR (local: {path})"),
                        };
                    }
                    Err(e) => warn!(error = %e, "DETR layout model failed to load"),
                }
            }

            // Tier 2: word-feature ORT (local path)
            if !crate::settings::effective_or("LAYOUT_ORT_MODEL_PATH", "").is_empty() {
                match OrtLayoutClassifier::load() {
                    Ok(m) => {
                        let path = crate::settings::effective_or("LAYOUT_ORT_MODEL_PATH", "");
                        info!("Word-ORT layout classifier loaded");
                        return LayoutModel {
                            inner: LayoutModelInner::WordOrt(std::sync::Mutex::new(m)),
                            source_label: format!("Word-ORT (local: {path})"),
                        };
                    }
                    Err(e) => warn!(error = %e, "Word-ORT layout model failed to load"),
                }
            }

            debug!("No layout model configured, using heuristic classifier");
            LayoutModel {
                inner: LayoutModelInner::Heuristic,
                source_label: "heuristic".to_string(),
            }
        })
    }

    /// Human-readable description of which layout-model tier is active and where
    /// it came from. Surfaced to the UI on /config/onnx.
    pub fn source_label(&self) -> &str {
        &self.source_label
    }

    /// Returns true when an ORT-based model (DETR or word-ORT) is loaded.
    pub fn is_candle_loaded(&self) -> bool {
        matches!(
            self.inner,
            LayoutModelInner::Detr(_) | LayoutModelInner::WordOrt(_)
        )
    }

    /// Classify word spans.  `pdf_bytes` is required for the DETR path (page
    /// rendering); the other tiers ignore it.
    pub fn classify(&self, words: &[WordSpan], pdf_bytes: &[u8]) -> Vec<RegionTag> {
        match &self.inner {
            LayoutModelInner::Detr(m) => m
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .classify(words, pdf_bytes)
                .unwrap_or_else(|e| {
                    debug!(error = %e, "DETR classify failed, falling back to heuristic");
                    heuristic_classify(words)
                }),
            LayoutModelInner::WordOrt(m) => m
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .classify(words)
                .unwrap_or_else(|e| {
                    debug!(error = %e, "Word-ORT classify failed, falling back to heuristic");
                    heuristic_classify(words)
                }),
            LayoutModelInner::Heuristic => heuristic_classify(words),
        }
    }
}

// ── DETR layout model ─────────────────────────────────────────────────────────

struct DetrLayoutModel {
    session: Session,
    threshold: f32,
    /// Number of layout classes excluding background (typically 5 for PubLayNet).
    num_classes: usize,
}

impl DetrLayoutModel {
    fn load() -> anyhow::Result<Self> {
        let model_path = crate::settings::effective_or("LAYOUT_DETR_MODEL_PATH", "");
        if model_path.is_empty() {
            anyhow::bail!("LAYOUT_DETR_MODEL_PATH not set");
        }

        if !std::path::Path::new(&model_path).exists() {
            anyhow::bail!("DETR model not found at {}", model_path);
        }

        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .commit_from_file(&model_path)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let threshold = crate::settings::effective_f64("LAYOUT_DETR_THRESHOLD", 0.7) as f32;

        // cmarkea/detr-layout-detection: 11 classes
        let num_classes = crate::settings::effective_u64("LAYOUT_DETR_NUM_CLASSES", 11) as usize;

        Ok(DetrLayoutModel {
            session,
            threshold,
            num_classes,
        })
    }

    fn classify(&mut self, words: &[WordSpan], pdf_bytes: &[u8]) -> anyhow::Result<Vec<RegionTag>> {
        if words.is_empty() {
            return Ok(Vec::new());
        }

        let max_page = words.iter().map(|w| w.page).max().unwrap_or(1);
        let mut all_tags = vec![RegionTag::Text; words.len()];

        // Bind pdfium once per classify call
        let pdfium = bind_pdfium()?;
        let doc = pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| anyhow::anyhow!("pdfium load: {e}"))?;

        for page_num in 1..=max_page {
            let page_words: Vec<(usize, &WordSpan)> = words
                .iter()
                .enumerate()
                .filter(|(_, w)| w.page == page_num)
                .collect();

            if page_words.is_empty() {
                continue;
            }

            let page_idx = (page_num - 1) as u16;
            let page = doc
                .pages()
                .get(page_idx)
                .map_err(|e| anyhow::anyhow!("page {page_num}: {e}"))?;

            // Render at 800px wide (DETR standard short-side)
            let config = pdfium_render::prelude::PdfRenderConfig::new()
                .set_target_width(800)
                .set_maximum_height(1333);
            let bitmap = page
                .render_with_config(&config)
                .map_err(|e| anyhow::anyhow!("render page {page_num}: {e}"))?;
            let img = bitmap.as_image();

            // Detect regions
            let regions = self.run_detr(&img)?;

            // Assign words to detected regions
            for (word_idx, word) in &page_words {
                if let Some(bbox) = word.bbox {
                    // word bbox is in 0–1000 normalised space
                    let wx0 = bbox[0] as f32 / 1000.0;
                    let wy0 = bbox[1] as f32 / 1000.0;
                    let wx1 = bbox[2] as f32 / 1000.0;
                    let wy1 = bbox[3] as f32 / 1000.0;

                    if let Some(tag) = best_region(wx0, wy0, wx1, wy1, &regions) {
                        all_tags[*word_idx] = tag;
                    }
                }
            }
        }

        Ok(all_tags)
    }

    fn run_detr(&mut self, img: &image::DynamicImage) -> anyhow::Result<Vec<DetectedRegion>> {
        let (h, w) = (img.height(), img.width());

        // Build [1, 3, H, W] f32 ImageNet-normalised tensor (CHW layout)
        let rgb = img.to_rgb8();
        let mean = [0.485f32, 0.456, 0.406];
        let std = [0.229f32, 0.224, 0.225];
        let npixels = (h * w) as usize;
        let mut data = vec![0f32; 3 * npixels];

        for (y, row) in rgb.rows().enumerate() {
            for (x, px) in row.enumerate() {
                let idx = y * w as usize + x;
                for c in 0..3usize {
                    data[c * npixels + idx] = (px[c] as f32 / 255.0 - mean[c]) / std[c];
                }
            }
        }

        let shape = vec![1i64, 3, h as i64, w as i64];
        let pixel_values =
            Tensor::from_array((shape, data)).map_err(|e| anyhow::anyhow!("tensor: {e}"))?;

        let outputs = self
            .session
            .run(inputs!["pixel_values" => pixel_values])
            .map_err(|e| anyhow::anyhow!("DETR run: {e}"))?;

        // logits: [1, Q, C+1], pred_boxes: [1, Q, 4]
        let (logits_shape, logits_flat) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("logits: {e}"))?;
        let (_, boxes_flat) = outputs[1]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("boxes: {e}"))?;

        let logits_shape: Vec<i64> = logits_shape.iter().copied().collect();
        let num_queries = logits_shape.get(1).copied().unwrap_or(0) as usize;
        let num_cls = self.num_classes + 1; // +1 for background

        let logits: Vec<f32> = logits_flat.iter().copied().collect();
        let boxes: Vec<f32> = boxes_flat.iter().copied().collect();

        let mut regions = Vec::new();

        for q in 0..num_queries {
            let logit_base = q * num_cls;
            if logit_base + num_cls > logits.len() {
                break;
            }

            // Softmax over this query's class logits
            let slice = &logits[logit_base..logit_base + num_cls];
            let max_l = slice.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exps: Vec<f32> = slice.iter().map(|&l| (l - max_l).exp()).collect();
            let sum: f32 = exps.iter().sum();
            let probs: Vec<f32> = exps.iter().map(|&e| e / sum).collect();

            // Best class among real classes (0..num_classes); index num_classes = no-object
            let no_object = self.num_classes;
            let Some((best_cls, &best_prob)) = probs[..no_object]
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
            else {
                continue;
            };

            if best_prob < self.threshold {
                continue;
            }
            let cls_idx = best_cls;

            // Decode box: (cx, cy, w, h) normalised → (x0, y0, x1, y1)
            let box_base = q * 4;
            if box_base + 4 > boxes.len() {
                break;
            }
            let cx = boxes[box_base];
            let cy = boxes[box_base + 1];
            let bw = boxes[box_base + 2];
            let bh = boxes[box_base + 3];
            let x0 = (cx - bw / 2.0).clamp(0.0, 1.0);
            let y0 = (cy - bh / 2.0).clamp(0.0, 1.0);
            let x1 = (cx + bw / 2.0).clamp(0.0, 1.0);
            let y1 = (cy + bh / 2.0).clamp(0.0, 1.0);

            regions.push(DetectedRegion {
                x0,
                y0,
                x1,
                y1,
                tag: detr_class_to_region(cls_idx),
                confidence: best_prob,
            });
        }

        Ok(regions)
    }
}

#[derive(Debug)]
struct DetectedRegion {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    tag: RegionTag,
    #[allow(dead_code)]
    confidence: f32,
}

/// Map cmarkea/detr-layout-detection class index to RegionTag.
/// Labels: 0=Caption 1=Footnote 2=Formula 3=List-item 4=Page-footer
///         5=Page-header 6=Picture 7=Section-header 8=Table 9=Text 10=Title
///         11=no-object (filtered before this call)
fn detr_class_to_region(cls: usize) -> RegionTag {
    match cls {
        0 => RegionTag::Caption,
        1 | 2 => RegionTag::Other,
        3 => RegionTag::List,
        4 => RegionTag::Footer,
        5 => RegionTag::Header,
        6 => RegionTag::Figure,
        7 => RegionTag::SectionHeader,
        8 => RegionTag::Table,
        9 => RegionTag::Text,
        10 => RegionTag::Title,
        _ => RegionTag::Text,
    }
}

/// Find the detected region that best contains the word bbox.
/// Uses centre-point containment first; falls back to highest-IoU region.
fn best_region(
    wx0: f32,
    wy0: f32,
    wx1: f32,
    wy1: f32,
    regions: &[DetectedRegion],
) -> Option<RegionTag> {
    let cx = (wx0 + wx1) / 2.0;
    let cy = (wy0 + wy1) / 2.0;

    // Prefer a region whose bbox contains the word's centre point
    if let Some(r) = regions
        .iter()
        .find(|r| cx >= r.x0 && cx <= r.x1 && cy >= r.y0 && cy <= r.y1)
    {
        return Some(r.tag);
    }

    // Fall back to highest-IoU region (catches partial overlaps)
    regions
        .iter()
        .map(|r| {
            let ix0 = wx0.max(r.x0);
            let iy0 = wy0.max(r.y0);
            let ix1 = wx1.min(r.x1);
            let iy1 = wy1.min(r.y1);
            let inter = ((ix1 - ix0).max(0.0)) * ((iy1 - iy0).max(0.0));
            let union = (wx1 - wx0) * (wy1 - wy0) + (r.x1 - r.x0) * (r.y1 - r.y0) - inter;
            (r, if union > 0.0 { inter / union } else { 0.0 })
        })
        .filter(|(_, iou)| *iou > 0.1)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(r, _)| r.tag)
}

fn bind_pdfium() -> anyhow::Result<pdfium_render::prelude::Pdfium> {
    use pdfium_render::prelude::Pdfium;

    // Try explicit path first, then system library
    let binding = if let Ok(path) = std::env::var("PDFIUM_LIBRARY_PATH") {
        Pdfium::bind_to_library(&path).map_err(|e| anyhow::anyhow!("pdfium at {path}: {e}"))?
    } else {
        Pdfium::bind_to_system_library().map_err(|e| {
            anyhow::anyhow!(
                "PDFium library not found. Set PDFIUM_LIBRARY_PATH or install libpdfium. \
                 Download from github.com/bblanchon/pdfium-binaries: {e}"
            )
        })?
    };

    Ok(Pdfium::new(binding))
}

// ── Word-feature ORT classifier ───────────────────────────────────────────────

struct OrtLayoutClassifier {
    session: Session,
}

impl OrtLayoutClassifier {
    fn load() -> anyhow::Result<Self> {
        let model_path = crate::settings::effective_or("LAYOUT_ORT_MODEL_PATH", "");
        if model_path.is_empty() {
            anyhow::bail!("LAYOUT_ORT_MODEL_PATH not set");
        }

        if !std::path::Path::new(&model_path).exists() {
            anyhow::bail!("ORT layout model not found at {}", model_path);
        }

        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .commit_from_file(&model_path)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(OrtLayoutClassifier { session })
    }

    fn classify(&mut self, words: &[WordSpan]) -> anyhow::Result<Vec<RegionTag>> {
        if words.is_empty() {
            return Ok(Vec::new());
        }

        let n = words.len();
        let mut features = vec![0f32; n * 8];

        for (i, word) in words.iter().enumerate() {
            let base = i * 8;
            if let Some(bb) = word.bbox {
                let x0 = (bb[0] as f32 / 1000.0).clamp(0.0, 1.0);
                let y0 = (bb[1] as f32 / 1000.0).clamp(0.0, 1.0);
                let x1 = (bb[2] as f32 / 1000.0).clamp(0.0, 1.0);
                let y1 = (bb[3] as f32 / 1000.0).clamp(0.0, 1.0);
                features[base] = x0;
                features[base + 1] = y0;
                features[base + 2] = x1;
                features[base + 3] = y1;
                features[base + 4] = (x1 - x0).max(0.0);
                features[base + 5] = (y1 - y0).max(0.0);
            }
            features[base + 6] = (word.text.len() as f32 / 50.0).min(1.0);
            features[base + 7] = (word.page as f32 / 100.0).min(1.0);
        }

        let shape = vec![1i64, n as i64, 8i64];
        let input =
            Tensor::from_array((shape, features)).map_err(|e| anyhow::anyhow!("tensor: {e}"))?;

        let outputs = self
            .session
            .run(inputs!["word_features" => input])
            .map_err(|e| anyhow::anyhow!("ORT run: {e}"))?;

        let (shape, data) = outputs[0]
            .try_extract_tensor::<i64>()
            .map_err(|e| anyhow::anyhow!("extract labels: {e}"))?;
        let flat: Vec<i64> = data.iter().copied().collect();
        let shape_dims: Vec<i64> = shape.iter().copied().collect();

        let labels: &[i64] = match shape_dims.as_slice() {
            [1, n] => &flat[..*n as usize],
            [n] => &flat[..*n as usize],
            _ => flat.as_slice(),
        };

        Ok(labels
            .iter()
            .map(|&l| word_ort_label_to_region(l))
            .collect())
    }
}

fn word_ort_label_to_region(label: i64) -> RegionTag {
    match label {
        1 => RegionTag::Title,
        2 => RegionTag::SectionHeader,
        3 => RegionTag::Table,
        4 => RegionTag::Figure,
        5 => RegionTag::List,
        6 => RegionTag::Footer,
        7 => RegionTag::Header,
        8 => RegionTag::Caption,
        9 => RegionTag::Other,
        _ => RegionTag::Text,
    }
}

// ── Heuristic classifier ──────────────────────────────────────────────────────

pub fn heuristic_classify(words: &[WordSpan]) -> Vec<RegionTag> {
    if words.is_empty() {
        return Vec::new();
    }

    let lines = group_into_lines(words);
    let page_count = words.iter().map(|w| w.page).max().unwrap_or(1);
    let mut tags: Vec<RegionTag> = vec![RegionTag::Text; words.len()];

    for (word_indices, text, bbox_y, word_count) in &lines {
        let tag = classify_line(text, *bbox_y, *word_count, page_count);
        for &wi in word_indices {
            tags[wi] = tag;
        }
    }

    tags
}

fn group_into_lines(words: &[WordSpan]) -> Vec<(Vec<usize>, String, i64, usize)> {
    let mut lines: Vec<(Vec<usize>, String, i64, usize)> = Vec::new();

    for (i, word) in words.iter().enumerate() {
        let y = word.bbox.map(|b| b[1]).unwrap_or(0);

        if let Some(last) = lines.last_mut() {
            if (y - last.2).abs() <= 25 {
                last.0.push(i);
                if !last.1.is_empty() {
                    last.1.push(' ');
                }
                last.1.push_str(&word.text);
                last.3 += 1;
                continue;
            }
        }

        lines.push((vec![i], word.text.clone(), y, 1));
    }

    lines
}

fn classify_line(text: &str, y: i64, word_count: usize, page_count: u32) -> RegionTag {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return RegionTag::Other;
    }

    if y > 900 || (y < 60 && page_count > 1) {
        return RegionTag::Footer;
    }

    let pipe_count = trimmed.chars().filter(|&c| c == '|').count();
    if pipe_count >= 2 || trimmed.contains('\t') {
        return RegionTag::Table;
    }

    if trimmed.starts_with(['•', '◦', '▪', '‣', '-', '*'])
        || trimmed
            .split_whitespace()
            .next()
            .map(|w| {
                w.ends_with('.')
                    && w.len() <= 4
                    && w[..w.len() - 1].chars().all(|c| c.is_ascii_digit())
            })
            .unwrap_or(false)
    {
        return RegionTag::List;
    }

    if word_count <= 10 && !trimmed.ends_with('.') {
        let alpha_words: Vec<&str> = trimmed
            .split_whitespace()
            .filter(|w| w.chars().any(|c| c.is_alphabetic()))
            .collect();
        if !alpha_words.is_empty() {
            let cap_ratio = alpha_words
                .iter()
                .filter(|w| w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false))
                .count() as f32
                / alpha_words.len() as f32;
            if cap_ratio >= 0.7 && y < 200 {
                return RegionTag::Title;
            }
            if cap_ratio >= 0.8 {
                return RegionTag::SectionHeader;
            }
        }
    }

    let lower = trimmed.to_lowercase();
    if lower.starts_with("figure ")
        || lower.starts_with("fig. ")
        || lower.starts_with("fig.")
        || lower.starts_with("table ")
        || lower.starts_with("algorithm ")
        || lower.starts_with("listing ")
    {
        return RegionTag::Caption;
    }

    RegionTag::Text
}
