use std::path::{Component, Path, PathBuf};

const MAX_UNFILTERED_UI_ROWS: usize = 400;
const MAX_FILTERED_UI_ROWS: usize = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerEntryKind {
    ParentDir,
    Directory,
    PdfFile,
    OtherFile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerEntry {
    pub name: String,
    pub path: PathBuf,
    pub kind: PickerEntryKind,
    pub is_hidden: bool,
    pub extension_badge: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerAction {
    NavigatedDirectory,
    OpenPdf(PathBuf),
    ShowError(String),
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn expand_tilde_path(raw: &str, home: Option<&Path>) -> PathBuf {
    if raw == "~" {
        return home.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
        && let Some(home) = home
    {
        return home.join(rest);
    }
    PathBuf::from(raw)
}

pub fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut prefix: Option<std::ffi::OsString> = None;
    let mut has_root = false;
    let mut parts: Vec<std::ffi::OsString> = Vec::new();

    for comp in path.components() {
        match comp {
            Component::Prefix(p) => {
                prefix = Some(p.as_os_str().to_os_string());
            }
            Component::RootDir => {
                has_root = true;
                parts.clear();
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(last) = parts.last() {
                    if last != ".." {
                        parts.pop();
                    } else if !has_root {
                        parts.push("..".into());
                    }
                } else if !has_root {
                    parts.push("..".into());
                }
            }
            Component::Normal(c) => parts.push(c.to_os_string()),
        }
    }

    let mut out = PathBuf::new();
    if let Some(p) = prefix {
        out.push(p);
    }
    if has_root {
        out.push("/");
    }
    for part in parts {
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

pub fn format_dir_input(dir: &Path) -> String {
    let norm = normalize_lexical_path(dir);
    let mut s = norm.to_string_lossy().into_owned();
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

pub fn sanitize_pasted_path(raw: &str) -> String {
    let cleaned_ctrl: String = raw
        .chars()
        .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    let trimmed = cleaned_ctrl.trim();
    let unquoted = if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    if let Some(url_rest) = unquoted
        .strip_prefix("file://localhost")
        .or_else(|| unquoted.strip_prefix("file://"))
    {
        let decoded = percent_decode_utf8(url_rest);
        // Strip leading `/` before Windows drive letters like `/C:/foo.pdf` if needed;
        // on macOS/Unix `file:///Users/...` gives `url_rest = "/Users/..."`, which stays intact.
        let bytes = decoded.as_bytes();
        if bytes.len() >= 4
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b':'
            && (bytes[3] == b'/' || bytes[3] == b'\\')
        {
            return decoded[1..].to_string();
        }
        return decoded;
    }

    unquoted.to_string()
}

fn percent_decode_utf8(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn is_absolute_or_home_path(s: &str) -> bool {
    let b = s.as_bytes();
    s.starts_with('/')
        || s == "~"
        || s.starts_with("~/")
        || s.starts_with("~\\")
        || (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'/' || b[2] == b'\\'))
}

pub fn resolve_picker_query(
    input: &str,
    fallback_dir: &Path,
    home: Option<&Path>,
) -> (PathBuf, String) {
    let trimmed_all = input.trim();
    if trimmed_all.is_empty() {
        return (normalize_lexical_path(fallback_dir), String::new());
    }

    let trimmed = input.trim_start();
    if trimmed == "~" || trimmed == "~/" || trimmed == "~\\" {
        return (
            normalize_lexical_path(&expand_tilde_path("~", home)),
            String::new(),
        );
    }

    if !trimmed.contains('/') && !trimmed.contains('\\') {
        return (normalize_lexical_path(fallback_dir), input.to_string());
    }

    let expanded = if trimmed.starts_with("~/") || trimmed.starts_with("~\\") {
        expand_tilde_path(trimmed, home).to_string_lossy().into_owned()
    } else {
        trimmed.to_string()
    };

    if expanded.ends_with('/') {
        let dir_path = Path::new(&expanded);
        let joined = if dir_path.is_absolute() {
            dir_path.to_path_buf()
        } else {
            fallback_dir.join(dir_path)
        };
        return (normalize_lexical_path(&joined), String::new());
    }

    if let Some(pos) = expanded.rfind('/') {
        let dir_part = if pos == 0 {
            "/"
        } else {
            &expanded[..=pos]
        };
        let filter_part = &expanded[pos + 1..];
        let dir_path = Path::new(dir_part);
        let joined = if dir_path.is_absolute() {
            dir_path.to_path_buf()
        } else {
            fallback_dir.join(dir_path)
        };
        return (normalize_lexical_path(&joined), filter_part.to_string());
    }

    (normalize_lexical_path(fallback_dir), expanded)
}

pub struct FilePickerState {
    pub input: String,
    pub cursor: usize,
    pub select_all: bool,
    pub selected_idx: usize,
    pub show_all_files: bool,
    pub fallback_dir: PathBuf,
    pub cached_dir: Option<PathBuf>,
    pub cached_dir_exists: bool,
    pub cached_entries: Vec<PickerEntry>,
    pub error_message: Option<String>,
}

impl FilePickerState {
    pub fn new(initial_dir: PathBuf) -> Self {
        let norm = normalize_lexical_path(&initial_dir);
        let input = format_dir_input(&norm);
        let cursor = input.len();
        let mut state = Self {
            input,
            cursor,
            select_all: false,
            selected_idx: 0,
            show_all_files: false,
            fallback_dir: norm,
            cached_dir: None,
            cached_dir_exists: false,
            cached_entries: Vec::new(),
            error_message: None,
        };
        state.sync_directory_cache();
        state.reset_selection_to_default();
        state
    }

    pub fn open_at(&mut self, dir: PathBuf, force_reload: bool) {
        if force_reload {
            self.cached_dir = None;
        }
        self.set_directory(&dir);
    }

    pub fn set_directory(&mut self, dir: &Path) {
        let norm = normalize_lexical_path(dir);
        self.fallback_dir = norm.clone();
        self.input = format_dir_input(&norm);
        self.cursor = self.input.len();
        self.select_all = false;
        self.error_message = None;
        self.sync_directory_cache();
        self.reset_selection_to_default();
    }

    pub fn sync_directory_cache(&mut self) {
        let (resolved_dir, _) =
            resolve_picker_query(&self.input, &self.fallback_dir, home_dir().as_deref());
        if self.cached_dir.as_ref() == Some(&resolved_dir) {
            return;
        }

        let mut entries = Vec::new();
        let exists = match std::fs::read_dir(&resolved_dir) {
            Ok(rd) => {
                for item in rd.flatten() {
                    let name_os = item.file_name();
                    let name = name_os.to_string_lossy().into_owned();
                    if name == "." || name == ".." || name == ".DS_Store" {
                        continue;
                    }
                    let path = item.path();
                    let is_dir = match item.file_type() {
                        Ok(ft) if ft.is_dir() => true,
                        Ok(ft) if ft.is_file() => false,
                        Ok(ft) if ft.is_symlink() => {
                            let Ok(meta) = std::fs::metadata(&path) else {
                                continue;
                            };
                            meta.is_dir()
                        }
                        Ok(_) => continue,
                        Err(_) => {
                            let Ok(meta) = std::fs::metadata(&path) else {
                                continue;
                            };
                            meta.is_dir()
                        }
                    };
                    let is_hidden = name.starts_with('.');
                    let (kind, extension_badge) = if is_dir {
                        (PickerEntryKind::Directory, "DIR".to_string())
                    } else if is_pdf_path(&path) {
                        (PickerEntryKind::PdfFile, "PDF".to_string())
                    } else {
                        let badge = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.trim().to_ascii_uppercase())
                            .filter(|e| !e.is_empty())
                            .map(|e| {
                                if e.chars().count() > 6 {
                                    e.chars().take(6).collect()
                                } else {
                                    e
                                }
                            })
                            .unwrap_or_else(|| "FILE".to_string());
                        (PickerEntryKind::OtherFile, badge)
                    };
                    entries.push(PickerEntry {
                        name,
                        path,
                        kind,
                        is_hidden,
                        extension_badge,
                    });
                }
                true
            }
            Err(_) => false,
        };

        entries.sort_by(|a, b| {
            kind_sort_group(a.kind)
                .cmp(&kind_sort_group(b.kind))
                .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
                .then_with(|| a.name.cmp(&b.name))
        });

        self.cached_dir = Some(resolved_dir);
        self.cached_dir_exists = exists;
        self.cached_entries = entries;
    }

    pub fn resolved_dir_and_filter(&self) -> (PathBuf, String) {
        resolve_picker_query(&self.input, &self.fallback_dir, home_dir().as_deref())
    }

    pub fn visible_entries(&self) -> Vec<PickerEntry> {
        self.visible_entries_with_total().0
    }

    pub fn visible_entries_with_total(&self) -> (Vec<PickerEntry>, usize) {
        let (resolved_dir, filter_text) = self.resolved_dir_and_filter();
        let filter_trimmed = filter_text.trim();
        let show_hidden = self.show_all_files || filter_text.starts_with('.');

        let mut out = Vec::new();
        if self.cached_dir_exists
            && let Some(parent) = resolved_dir.parent()
            && (filter_trimmed.is_empty() || filter_trimmed == "." || filter_trimmed == "..")
        {
            out.push(PickerEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                kind: PickerEntryKind::ParentDir,
                is_hidden: false,
                extension_badge: "UP".to_string(),
            });
        }

        if filter_trimmed.is_empty() {
            for entry in &self.cached_entries {
                let hidden_entry = entry.is_hidden || entry.name == "__pycache__";
                if hidden_entry && !show_hidden {
                    continue;
                }
                if entry.kind == PickerEntryKind::OtherFile && !self.show_all_files {
                    continue;
                }
                out.push(entry.clone());
            }
            let total = out.len();
            out.truncate(MAX_UNFILTERED_UI_ROWS);
            return (out, total);
        }

        let tokens: Vec<String> = filter_trimmed
            .split_whitespace()
            .map(|t| t.to_ascii_lowercase())
            .collect();
        let query_lower = filter_trimmed.to_ascii_lowercase();
        let mut scored: Vec<(u8, usize, String, PickerEntry)> = Vec::new();

        for entry in &self.cached_entries {
            let allow_pycache = entry.name == "__pycache__" && !filter_trimmed.is_empty();
            if entry.is_hidden && !show_hidden {
                continue;
            }
            if entry.name == "__pycache__" && !self.show_all_files && !allow_pycache {
                continue;
            }
            if entry.kind == PickerEntryKind::OtherFile && !self.show_all_files {
                continue;
            }
            let Some(score) = match_score(&entry.name, entry.kind, &query_lower, &tokens) else {
                continue;
            };
            scored.push((
                score,
                entry.name.len(),
                entry.name.to_ascii_lowercase(),
                entry.clone(),
            ));
        }

        scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)).then_with(|| a.2.cmp(&b.2)));
        out.extend(scored.into_iter().map(|(_, _, _, e)| e));
        let total = out.len();
        out.truncate(MAX_FILTERED_UI_ROWS);
        (out, total)
    }

    pub fn default_selected_index(entries: &[PickerEntry], filter_text: &str) -> usize {
        if filter_text.trim().is_empty()
            && entries.len() > 1
            && entries[0].kind == PickerEntryKind::ParentDir
        {
            1
        } else {
            0
        }
    }

    fn reset_selection_to_default(&mut self) {
        let (_, filter_text) = self.resolved_dir_and_filter();
        let entries = self.visible_entries();
        self.selected_idx = Self::default_selected_index(&entries, &filter_text);
    }

    fn on_input_edited(&mut self) {
        self.error_message = None;
        self.sync_directory_cache();
        if self.cached_dir_exists
            && self.input.ends_with('/')
            && let Some(ref d) = self.cached_dir
        {
            self.fallback_dir = d.clone();
        }
        self.reset_selection_to_default();
    }

    pub fn select_all(&mut self) {
        if !self.input.is_empty() {
            self.select_all = true;
            self.cursor = self.input.len();
        }
    }

    pub fn move_left(&mut self) {
        self.select_all = false;
        if self.cursor == 0 {
            return;
        }
        if let Some((prev_idx, _)) = self.input[..self.cursor].char_indices().next_back() {
            self.cursor = prev_idx;
        } else {
            self.cursor = 0;
        }
    }

    pub fn move_right(&mut self) {
        self.select_all = false;
        if self.cursor >= self.input.len() {
            return;
        }
        if let Some((_, ch)) = self.input[self.cursor..].char_indices().next() {
            self.cursor += ch.len_utf8();
        } else {
            self.cursor = self.input.len();
        }
    }

    pub fn move_home(&mut self) {
        self.select_all = false;
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.select_all = false;
        self.cursor = self.input.len();
    }

    pub fn clear_input(&mut self) -> bool {
        if self.input.is_empty() && !self.select_all {
            return false;
        }
        self.input.clear();
        self.cursor = 0;
        self.select_all = false;
        self.on_input_edited();
        true
    }

    pub fn backspace(&mut self) -> bool {
        if self.select_all {
            return self.clear_input();
        }
        if self.cursor == 0 {
            return false;
        }
        let Some((prev_idx, _)) = self.input[..self.cursor].char_indices().next_back() else {
            return false;
        };
        self.input.drain(prev_idx..self.cursor);
        self.cursor = prev_idx;
        self.on_input_edited();
        true
    }

    pub fn delete_forward(&mut self) -> bool {
        if self.select_all {
            return self.clear_input();
        }
        if self.cursor >= self.input.len() {
            return false;
        }
        let Some((_, ch)) = self.input[self.cursor..].char_indices().next() else {
            return false;
        };
        self.input.drain(self.cursor..self.cursor + ch.len_utf8());
        self.on_input_edited();
        true
    }

    pub fn delete_prev_segment(&mut self) -> bool {
        if self.select_all {
            return self.clear_input();
        }
        if self.cursor == 0 {
            return false;
        }
        let mut cut_end = self.cursor;
        if cut_end > 1 && self.input.as_bytes()[cut_end - 1] == b'/' {
            cut_end -= 1;
        }
        let prefix = &self.input[..cut_end];
        let cut_start = match prefix.rfind('/') {
            Some(pos) => pos + 1,
            None => 0,
        };
        if cut_start >= self.cursor {
            return self.backspace();
        }
        self.input.drain(cut_start..self.cursor);
        self.cursor = cut_start;
        self.on_input_edited();
        true
    }

    pub fn insert_text(&mut self, text: &str) -> bool {
        let filtered: String = text.chars().filter(|c| !c.is_control()).collect();
        if filtered.is_empty() {
            return false;
        }
        if self.select_all {
            self.input = filtered;
            self.cursor = self.input.len();
            self.select_all = false;
        } else {
            let idx = self.cursor.min(self.input.len());
            self.input.insert_str(idx, &filtered);
            self.cursor = idx + filtered.len();
        }
        self.on_input_edited();
        true
    }

    pub fn apply_paste(&mut self, raw_clipboard: &str) -> bool {
        let cleaned = sanitize_pasted_path(raw_clipboard);
        if cleaned.is_empty() {
            return false;
        }
        let home = home_dir();
        if self.select_all || is_absolute_or_home_path(&cleaned) {
            let expanded = expand_tilde_path(&cleaned, home.as_deref());
            let mut next_input = cleaned;
            if expanded.is_dir() && !next_input.ends_with('/') {
                next_input.push('/');
            }
            self.input = next_input;
            self.cursor = self.input.len();
            self.select_all = false;
            self.on_input_edited();
            return true;
        }
        self.insert_text(&cleaned)
    }

    pub fn tab_complete(&mut self) -> bool {
        let entries = self.visible_entries();
        if entries.is_empty() {
            return false;
        }
        let item = &entries[self.selected_idx.min(entries.len() - 1)];
        match item.kind {
            PickerEntryKind::ParentDir | PickerEntryKind::Directory => {
                self.set_directory(&item.path);
                true
            }
            PickerEntryKind::PdfFile | PickerEntryKind::OtherFile => {
                self.input = item.path.to_string_lossy().into_owned();
                self.cursor = self.input.len();
                self.select_all = false;
                self.on_input_edited();
                true
            }
        }
    }

    pub fn confirm_selection(&mut self) -> PickerAction {
        self.sync_directory_cache();
        let entries = self.visible_entries();
        if let Some(entry) = entries.get(self.selected_idx.min(entries.len().saturating_sub(1))) {
            return self.activate_entry(entry);
        }

        let trimmed = self.input.trim();
        if trimmed.is_empty() {
            let msg = "Enter a folder path or PDF file path".to_string();
            self.error_message = Some(msg.clone());
            return PickerAction::ShowError(msg);
        }

        let raw_path = expand_tilde_path(trimmed, home_dir().as_deref());
        let resolved = if raw_path.is_absolute() {
            normalize_lexical_path(&raw_path)
        } else {
            normalize_lexical_path(&self.fallback_dir.join(&raw_path))
        };

        if resolved.is_dir() {
            self.set_directory(&resolved);
            return PickerAction::NavigatedDirectory;
        }
        if resolved.is_file() {
            if is_pdf_path(&resolved) {
                return PickerAction::OpenPdf(resolved);
            }
            let msg = format!(
                "Not a PDF file: {}",
                resolved
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("selected item")
            );
            self.error_message = Some(msg.clone());
            return PickerAction::ShowError(msg);
        }

        let msg = if !self.cached_dir_exists {
            let missing = self
                .cached_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| resolved.display().to_string());
            format!("Folder not found: {missing}")
        } else {
            format!("Path not found: {}", resolved.display())
        };
        self.error_message = Some(msg.clone());
        PickerAction::ShowError(msg)
    }

    pub fn activate_entry(&mut self, entry: &PickerEntry) -> PickerAction {
        match entry.kind {
            PickerEntryKind::ParentDir | PickerEntryKind::Directory => {
                self.set_directory(&entry.path);
                PickerAction::NavigatedDirectory
            }
            PickerEntryKind::PdfFile => PickerAction::OpenPdf(entry.path.clone()),
            PickerEntryKind::OtherFile => {
                let msg = format!("Pinleaf only opens PDF documents ({})", entry.name);
                self.error_message = Some(msg.clone());
                PickerAction::ShowError(msg)
            }
        }
    }

    pub fn format_input_for_display(&self, max_chars: usize) -> String {
        let with_caret = if self.select_all {
            self.input.clone()
        } else {
            let idx = self.cursor.min(self.input.len());
            let mut s = String::with_capacity(self.input.len() + 1);
            s.push_str(&self.input[..idx]);
            s.push('|');
            s.push_str(&self.input[idx..]);
            s
        };

        let chars: Vec<char> = with_caret.chars().collect();
        if chars.len() <= max_chars || max_chars < 8 {
            return with_caret;
        }

        let caret_char_idx = if self.select_all {
            chars.len()
        } else {
            self.input[..self.cursor.min(self.input.len())]
                .chars()
                .count()
        };
        let body_budget = max_chars.saturating_sub(3);
        let half = body_budget / 2;

        if caret_char_idx >= chars.len().saturating_sub(half) {
            let tail_start = chars.len() - body_budget;
            let tail: String = chars[tail_start..].iter().collect();
            format!("...{tail}")
        } else if caret_char_idx <= body_budget {
            let head: String = chars[..body_budget].iter().collect();
            format!("{head}...")
        } else {
            let window_start = caret_char_idx.saturating_sub(half);
            let inner_budget = max_chars.saturating_sub(6);
            let window_end = (window_start + inner_budget).min(chars.len());
            let slice: String = chars[window_start..window_end].iter().collect();
            format!("...{slice}...")
        }
    }
}

pub fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn kind_sort_group(kind: PickerEntryKind) -> u8 {
    match kind {
        PickerEntryKind::ParentDir => 0,
        PickerEntryKind::Directory => 1,
        PickerEntryKind::PdfFile => 2,
        PickerEntryKind::OtherFile => 3,
    }
}

fn match_score(
    name: &str,
    kind: PickerEntryKind,
    query_lower: &str,
    tokens: &[String],
) -> Option<u8> {
    let lower = name.to_ascii_lowercase();
    let other_penalty = u8::from(kind == PickerEntryKind::OtherFile) * 5;
    if lower == query_lower {
        return Some(u8::from(kind == PickerEntryKind::OtherFile));
    }
    if lower.starts_with(query_lower) {
        return Some(10 + other_penalty);
    }
    if tokens.iter().all(|tok| lower.contains(tok)) {
        return Some(20 + other_penalty);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "pinleaf_picker_test_{label}_{}_{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_picker_query_handles_dirs_prefixes_tilde_and_bare_filters() {
        let home = PathBuf::from("/Users/tester");
        let fallback = PathBuf::from("/work/papers");

        assert_eq!(
            resolve_picker_query("/Users/tester/docs/", &fallback, Some(&home)),
            (PathBuf::from("/Users/tester/docs"), String::new())
        );
        assert_eq!(
            resolve_picker_query("/Users/tester/docs/sub", &fallback, Some(&home)),
            (PathBuf::from("/Users/tester/docs"), "sub".to_string())
        );
        assert_eq!(
            resolve_picker_query("~/docs/paper", &fallback, Some(&home)),
            (PathBuf::from("/Users/tester/docs"), "paper".to_string())
        );
        assert_eq!(
            resolve_picker_query("~/", &fallback, Some(&home)),
            (PathBuf::from("/Users/tester"), String::new())
        );
        assert_eq!(
            resolve_picker_query("forest", &fallback, Some(&home)),
            (PathBuf::from("/work/papers"), "forest".to_string())
        );
        assert_eq!(
            resolve_picker_query("/etc", &fallback, Some(&home)),
            (PathBuf::from("/"), "etc".to_string())
        );
        assert_eq!(
            resolve_picker_query("/", &fallback, Some(&home)),
            (PathBuf::from("/"), String::new())
        );
    }

    #[test]
    fn sanitize_pasted_path_strips_quotes_newlines_and_file_urls() {
        assert_eq!(
            sanitize_pasted_path("  '/PCDC_Core/xzhan9/run/ecoli'\n"),
            "/PCDC_Core/xzhan9/run/ecoli"
        );
        assert_eq!(
            sanitize_pasted_path("\"/Users/a/My Paper.pdf\""),
            "/Users/a/My Paper.pdf"
        );
        assert_eq!(
            sanitize_pasted_path("file:///Users/test/My%20Paper.pdf"),
            "/Users/test/My Paper.pdf"
        );
        assert_eq!(
            sanitize_pasted_path("file://localhost/tmp/draft%20v2.pdf"),
            "/tmp/draft v2.pdf"
        );
    }

    #[test]
    fn file_picker_lists_filters_pastes_segments_and_confirms() {
        let root = make_temp_dir("workflow");
        let sub = root.join("validation");
        let other_dir = root.join("samples");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(&other_dir).unwrap();
        let pdf_a = root.join("association_forest.pdf");
        let pdf_b = root.join("beta_appendix.pdf");
        let tsv_file = root.join("association_results.tsv");
        let dot_pdf = root.join(".hidden_draft.pdf");
        let sub_pdf = sub.join("nested_report.pdf");
        std::fs::write(&pdf_a, b"%PDF-1.4\n").unwrap();
        std::fs::write(&pdf_b, b"%PDF-1.4\n").unwrap();
        std::fs::write(&tsv_file, b"col1\tcol2\n").unwrap();
        std::fs::write(&dot_pdf, b"%PDF-1.4\n").unwrap();
        std::fs::write(&sub_pdf, b"%PDF-1.4\n").unwrap();

        let mut picker = FilePickerState::new(root.clone());
        let visible = picker.visible_entries();
        assert_eq!(visible[0].kind, PickerEntryKind::ParentDir);
        assert_eq!(visible[1].name, "samples");
        assert_eq!(visible[2].name, "validation");
        assert_eq!(visible[3].name, "association_forest.pdf");
        assert_eq!(visible[4].name, "beta_appendix.pdf");
        assert!(!visible.iter().any(|e| e.name == "association_results.tsv"));
        assert!(!visible.iter().any(|e| e.name == ".hidden_draft.pdf"));
        // Default selection skips `..` when folder has real children.
        assert_eq!(picker.selected_idx, 1);

        // Toggle All Files reveals non-PDF files & dotfiles.
        picker.show_all_files = true;
        let all_visible = picker.visible_entries();
        assert!(all_visible.iter().any(|e| e.name == "association_results.tsv"));
        assert!(all_visible.iter().any(|e| e.name == ".hidden_draft.pdf"));
        picker.show_all_files = false;

        // Live filtering narrows entries while typing.
        assert!(picker.insert_text("assoc"));
        let filtered = picker.visible_entries();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "association_forest.pdf");
        assert_eq!(picker.selected_idx, 0);
        assert_eq!(
            picker.confirm_selection(),
            PickerAction::OpenPdf(pdf_a.clone())
        );

        // Typing '.' reveals hidden dotfiles even with All Files off.
        assert!(picker.clear_input());
        assert!(picker.insert_text(&format!("{}/.", root.display())));
        let dot_filtered = picker.visible_entries();
        assert!(dot_filtered.iter().any(|e| e.name == ".hidden_draft.pdf"));

        // Pasting a directory path without trailing slash auto-appends '/' and lists its children.
        assert!(picker.apply_paste(&format!("  '{}'\n", sub.display())));
        assert_eq!(picker.input, format!("{}/", sub.display()));
        let sub_visible = picker.visible_entries();
        assert!(sub_visible.iter().any(|e| e.name == "nested_report.pdf"));

        // Alt+Backspace pops one path segment cleanly (`/root/validation/` -> `/root/`).
        assert!(picker.delete_prev_segment());
        assert_eq!(picker.input, format!("{}/", root.display()));

        // Selecting a directory and pressing Enter navigates into it.
        picker.selected_idx = 2; // "validation"
        assert_eq!(picker.confirm_selection(), PickerAction::NavigatedDirectory);
        assert_eq!(picker.input, format!("{}/", sub.display()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn substring_matching_finds_target_and_excludes_random_hex_hashes() {
        let query = "03847.pdf";
        let tokens = vec![query.to_string()];
        assert!(
            match_score(
                "CP-MULTI-JOURNAL-D-26-03847.pdf",
                PickerEntryKind::PdfFile,
                query,
                &tokens,
            )
            .is_some()
        );
        assert_eq!(
            match_score(
                "7ce2e14a0611c8ad4c0ed23aab26276f4068b934fdd3700c409ae70bb1d2fc29_optim.pdf",
                PickerEntryKind::PdfFile,
                query,
                &tokens,
            ),
            None
        );
    }

    #[test]
    fn full_scan_finds_pdf_beyond_5000_entries() {
        let root = make_temp_dir("large_scan");
        for i in 0..5_050 {
            let dummy = root.join(format!("dummy_{i:04}.txt"));
            let _ = std::fs::File::create(&dummy).unwrap();
        }
        let target = root.join("CP-MULTI-JOURNAL-D-26-03847.pdf");
        std::fs::write(&target, b"%PDF-1.4\n").unwrap();

        let mut picker = FilePickerState::new(root.clone());
        assert_eq!(picker.cached_entries.len(), 5_051);
        assert!(picker.insert_text("03847.pdf"));
        let matches = picker.visible_entries();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "CP-MULTI-JOURNAL-D-26-03847.pdf");

        let _ = std::fs::remove_dir_all(&root);
    }
}
