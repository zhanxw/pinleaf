use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use pinleaf_core::{NormPoint, NormRect, PageIndex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncTexSourceLoc {
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncTexPdfLoc {
    pub page: PageIndex,
    pub rect: NormRect,
}

#[derive(Clone, Debug)]
struct SyncTexEntry {
    page: PageIndex,
    input_tag: u32,
    line: u32,
    column: u32,
    rect: NormRect,
    is_leaf_or_hbox: bool,
}

#[derive(Clone, Debug)]
pub struct SyncTexMap {
    inputs: HashMap<u32, PathBuf>,
    entries: Vec<SyncTexEntry>,
}

impl SyncTexMap {
    pub fn load_for_pdf(pdf_path: &Path) -> Option<Self> {
        let base_dir = pdf_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let stem = pdf_path.with_extension("");
        let gz_path = PathBuf::from(format!("{}.synctex.gz", stem.display()));
        if let Ok(bytes) = std::fs::read(&gz_path) {
            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut text = String::new();
            if decoder.read_to_string(&mut text).is_ok()
                && let Some(parsed) = Self::parse_str(&text, base_dir)
            {
                return Some(parsed);
            }
        }

        let plain_path = PathBuf::from(format!("{}.synctex", stem.display()));
        if let Ok(text) = std::fs::read_to_string(&plain_path) {
            return Self::parse_str(&text, base_dir);
        }

        None
    }

    pub fn parse_str(content: &str, base_dir: &Path) -> Option<Self> {
        let mut inputs = HashMap::new();
        let mut entries = Vec::new();
        let mut mag: f64 = 1000.0;
        let mut unit: f64 = 1.0;
        let mut x_offset: f64 = 0.0;
        let mut y_offset: f64 = 0.0;
        let mut current_page: Option<PageIndex> = None;

        for raw_line in content.lines() {
            let line = raw_line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("Input:") {
                if let Some((tag_str, path_str)) = rest.split_once(':')
                    && let Ok(tag) = tag_str.trim().parse::<u32>()
                {
                    let p = Path::new(path_str.trim());
                    let resolved = if p.is_absolute() {
                        p.to_path_buf()
                    } else {
                        normalize_path(&base_dir.join(p))
                    };
                    inputs.insert(tag, resolved);
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("Magnification:") {
                if let Ok(v) = rest.trim().parse::<f64>()
                    && v > 0.0
                {
                    mag = v;
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("Unit:") {
                if let Ok(v) = rest.trim().parse::<f64>()
                    && v > 0.0
                {
                    unit = v;
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("X Offset:") {
                if let Ok(v) = rest.trim().parse::<f64>() {
                    x_offset = v;
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("Y Offset:") {
                if let Ok(v) = rest.trim().parse::<f64>() {
                    y_offset = v;
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix('{') {
                if let Ok(page_one_based) = rest.trim().parse::<u32>()
                    && page_one_based > 0
                {
                    current_page = Some(PageIndex(page_one_based - 1));
                }
                continue;
            }
            if line.starts_with('}') {
                current_page = None;
                continue;
            }

            let Some(page) = current_page else {
                continue;
            };
            let kind = line.chars().next().unwrap_or('\0');
            if !matches!(kind, '(' | '[' | 'h' | 'v' | 'x' | 'k' | 'g' | '$') {
                continue;
            }

            let body = &line[kind.len_utf8()..];
            let Some((link_part, geom_part)) = body.split_once(':') else {
                continue;
            };
            let Some((tag, src_line, src_col)) = parse_link_part(link_part) else {
                continue;
            };
            if src_line == 0 || !inputs.contains_key(&tag) {
                continue;
            }

            let scale_to_bp = (unit / 65781.76) * (mag / 1000.0);
            let (pos_part, size_part) = match geom_part.split_once(':') {
                Some((p, s)) => (p, Some(s)),
                None => (geom_part, None),
            };
            let Some((h_raw, v_raw)) = pos_part.split_once(',') else {
                continue;
            };
            let (Ok(h_sp), Ok(v_sp)) = (h_raw.trim().parse::<f64>(), v_raw.trim().parse::<f64>())
            else {
                continue;
            };
            let (w_bp, h_bp, d_bp) = parse_size_part(size_part, scale_to_bp);
            let x_bp = ((h_sp + x_offset) * scale_to_bp) as f32;
            let y_bp = ((v_sp + y_offset) * scale_to_bp) as f32;

            let min_x = x_bp.max(0.0);
            let max_x = (x_bp + w_bp.max(8.0)).max(min_x + 4.0);
            let min_y = (y_bp - h_bp.max(10.0)).max(0.0);
            let max_y = (y_bp + d_bp.max(3.0)).max(min_y + 4.0);

            entries.push(SyncTexEntry {
                page,
                input_tag: tag,
                line: src_line,
                column: src_col,
                rect: NormRect::new(min_x, min_y, max_x, max_y),
                is_leaf_or_hbox: matches!(kind, 'x' | 'k' | 'g' | '$' | 'h' | '('),
            });
        }

        if inputs.is_empty() || entries.is_empty() {
            return None;
        }
        Some(Self { inputs, entries })
    }

    pub fn inverse_search(&self, page: PageIndex, pt: NormPoint) -> Option<SyncTexSourceLoc> {
        let mut best: Option<(f32, &SyncTexEntry)> = None;
        for entry in &self.entries {
            if entry.page != page {
                continue;
            }
            let r = entry.rect;
            let dx = if pt.x < r.min_x {
                r.min_x - pt.x
            } else if pt.x > r.max_x {
                pt.x - r.max_x
            } else {
                0.0
            };
            let dy = if pt.y < r.min_y {
                r.min_y - pt.y
            } else if pt.y > r.max_y {
                pt.y - r.max_y
            } else {
                0.0
            };
            let kind_penalty = if entry.is_leaf_or_hbox { 0.0 } else { 12.0 };
            let score = dy * 2.5 + dx * 0.25 + kind_penalty;
            if best.is_none_or(|(bs, _)| score < bs) {
                best = Some((score, entry));
            }
        }
        let (_, chosen) = best?;
        let file = self.inputs.get(&chosen.input_tag)?.clone();
        Some(SyncTexSourceLoc {
            file,
            line: chosen.line,
            column: chosen.column,
        })
    }

    pub fn forward_search(&self, tex_path: &Path, target_line: u32) -> Option<SyncTexPdfLoc> {
        let matching_tags = self.matching_input_tags(tex_path);
        if matching_tags.is_empty() {
            return None;
        }

        let mut best_line_match: Option<(u32, bool, &SyncTexEntry)> = None;
        for entry in &self.entries {
            if !matching_tags.contains(&entry.input_tag) {
                continue;
            }
            let line_diff = entry.line.abs_diff(target_line);
            if line_diff > 5 {
                continue;
            }
            let better = match best_line_match {
                None => true,
                Some((best_diff, best_leaf, _)) => {
                    line_diff < best_diff || (line_diff == best_diff && entry.is_leaf_or_hbox && !best_leaf)
                }
            };
            if better {
                best_line_match = Some((line_diff, entry.is_leaf_or_hbox, entry));
            }
        }

        let (_, _, seed) = best_line_match?;
        let chosen_page = seed.page;
        let chosen_line = seed.line;
        let chosen_tag = seed.input_tag;

        let mut union_rect = seed.rect;
        for entry in &self.entries {
            if entry.page == chosen_page
                && entry.input_tag == chosen_tag
                && entry.line == chosen_line
                && entry.is_leaf_or_hbox
            {
                union_rect = union_rect.union(entry.rect);
            }
        }

        Some(SyncTexPdfLoc {
            page: chosen_page,
            rect: union_rect,
        })
    }

    fn matching_input_tags(&self, query: &Path) -> Vec<u32> {
        let query_canon = std::fs::canonicalize(query).ok();
        let query_norm = normalize_path(query);
        let query_name = query.file_name();

        let mut exact = Vec::new();
        let mut suffix = Vec::new();
        let mut by_name = Vec::new();

        for (&tag, candidate) in &self.inputs {
            if let Some(ref qc) = query_canon
                && let Ok(cc) = std::fs::canonicalize(candidate)
                && &cc == qc
            {
                exact.push(tag);
                continue;
            }
            if candidate == &query_norm {
                exact.push(tag);
                continue;
            }
            if candidate.ends_with(&query_norm) || query_norm.ends_with(candidate) {
                suffix.push(tag);
                continue;
            }
            if query_name.is_some() && candidate.file_name() == query_name {
                by_name.push(tag);
            }
        }

        if !exact.is_empty() {
            exact
        } else if !suffix.is_empty() {
            suffix
        } else {
            by_name
        }
    }
}

fn parse_link_part(raw: &str) -> Option<(u32, u32, u32)> {
    let mut parts = raw.split(',');
    let tag = parts.next()?.trim().parse::<u32>().ok()?;
    let line = parts.next()?.trim().parse::<u32>().ok()?;
    let col = parts
        .next()
        .and_then(|c| c.trim().parse::<u32>().ok())
        .unwrap_or(0);
    Some((tag, line, col))
}

fn parse_size_part(size_part: Option<&str>, scale_to_bp: f64) -> (f32, f32, f32) {
    let Some(raw) = size_part else {
        return (0.0, 0.0, 0.0);
    };
    let mut parts = raw.split(',');
    let w = parts
        .next()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
        * scale_to_bp;
    let h = parts
        .next()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
        * scale_to_bp;
    let d = parts
        .next()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
        * scale_to_bp;
    (w.max(0.0) as f32, h.max(0.0) as f32, d.max(0.0) as f32)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push(comp.as_os_str());
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;

    fn sample_synctex() -> String {
        let scale = 65781.76_f64;
        let sp = |bp: f64| -> i64 { (bp * scale).round() as i64 };
        format!(
            "SyncTeX Version:1\n\
Input:1:./main.tex\n\
Input:2:./sections/intro.tex\n\
Output:pdf\n\
Magnification:1000\n\
Unit:1\n\
X Offset:0\n\
Y Offset:0\n\
Content:\n\
{{1\n\
[1,1:{},{}:{},{},{}\n\
(2,42,3:{},{}:{},{},{}\n\
x2,42,5:{},{}\n\
x2,43,1:{},{}\n\
]\n\
}}\n\
{{2\n\
(2,108,1:{},{}:{},{},{}\n\
x2,108,4:{},{}\n\
}}\n",
            sp(72.0),
            sp(100.0),
            sp(400.0),
            sp(14.0),
            sp(3.0),
            sp(72.0),
            sp(140.0),
            sp(320.0),
            sp(12.0),
            sp(3.0),
            sp(110.0),
            sp(140.0),
            sp(72.0),
            sp(156.0),
            sp(72.0),
            sp(220.0),
            sp(300.0),
            sp(12.0),
            sp(3.0),
            sp(120.0),
            sp(220.0),
        )
    }

    #[test]
    fn synctex_parses_gzipped_and_resolves_forward_and_inverse() {
        let dir = std::env::temp_dir().join(format!("pinleaf-synctex-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf_path = dir.join("paper.pdf");
        let gz_path = dir.join("paper.synctex.gz");

        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(sample_synctex().as_bytes()).unwrap();
        let compressed = enc.finish().unwrap();
        std::fs::write(&gz_path, compressed).unwrap();

        let map = SyncTexMap::load_for_pdf(&pdf_path).expect("loads .synctex.gz");

        let fwd = map
            .forward_search(&dir.join("sections/intro.tex"), 42)
            .expect("forward search finds line 42");
        assert_eq!(fwd.page, PageIndex(0));
        assert!((fwd.rect.min_x - 72.0).abs() < 1.5);
        assert!(fwd.rect.max_y > 138.0 && fwd.rect.min_y < 140.0);

        let inv = map
            .inverse_search(PageIndex(1), NormPoint::new(122.0, 218.0))
            .expect("inverse search finds line 108 on page 2");
        assert_eq!(inv.line, 108);
        assert!(inv.file.ends_with("sections/intro.tex"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
