use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::session::ColorMode;

const MAX_RECENT_DOCS: usize = 200;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserConfig {
    #[serde(default)]
    pub default_color_mode: Option<ColorMode>,
    #[serde(default)]
    pub auto_reload: Option<bool>,
    #[serde(default)]
    pub keybindings: HashMap<String, String>,
}

impl UserConfig {
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("pinleaf").join("config.json"))
    }

    pub fn load() -> Self {
        Self::config_path()
            .map(|p| Self::load_from_path(&p))
            .unwrap_or_default()
    }

    pub fn load_from_path(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<UserConfig>(&raw).ok())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexedHighlight {
    pub page_one_based: u32,
    pub quote: String,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentDocEntry {
    pub path: PathBuf,
    pub title: String,
    pub last_opened_secs: u64,
    pub page_count: usize,
    pub last_page: u32,
    #[serde(default)]
    pub highlights: Vec<IndexedHighlight>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobalHighlightHit {
    pub path: Option<PathBuf>,
    pub doc_title: String,
    pub page_one_based: u32,
    pub quote: String,
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibraryStore {
    #[serde(default)]
    pub documents: Vec<RecentDocEntry>,
}

impl LibraryStore {
    pub fn default_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("pinleaf").join("library.json"))
    }

    pub fn load() -> Self {
        Self::default_path()
            .map(|p| Self::load_from_path(&p))
            .unwrap_or_default()
    }

    pub fn load_from_path(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<LibraryStore>(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(path) = Self::default_path() {
            self.save_to_path(&path)?;
        }
        Ok(())
    }

    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp_name = format!(
            ".{}.tmp-{}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("library"),
            std::process::id()
        );
        let tmp_path = path.with_file_name(tmp_name);
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn upsert_entry(&mut self, entry: RecentDocEntry) {
        if let Some(existing) = self.documents.iter_mut().find(|d| d.path == entry.path) {
            *existing = entry;
        } else {
            self.documents.push(entry);
        }
        self.documents
            .sort_by(|a, b| b.last_opened_secs.cmp(&a.last_opened_secs));
        self.documents.truncate(MAX_RECENT_DOCS);
    }

    pub fn filter_recent<'a>(&'a self, query: &str) -> Vec<&'a RecentDocEntry> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.documents.iter().collect();
        }
        let tokens: Vec<String> = trimmed
            .split_whitespace()
            .map(|t| t.to_ascii_lowercase())
            .collect();
        self.documents
            .iter()
            .filter(|doc| {
                let title_l = doc.title.to_ascii_lowercase();
                let path_l = doc.path.to_string_lossy().to_ascii_lowercase();
                tokens
                    .iter()
                    .all(|tok| title_l.contains(tok) || path_l.contains(tok))
            })
            .collect()
    }

    pub fn search_highlights(&self, query: &str) -> Vec<GlobalHighlightHit> {
        let tokens: Vec<String> = query
            .split_whitespace()
            .map(|t| t.to_ascii_lowercase())
            .collect();

        let mut hits = Vec::new();
        for doc in &self.documents {
            for hl in &doc.highlights {
                if tokens.is_empty() || matches_highlight(&doc.title, hl, &tokens) {
                    hits.push(GlobalHighlightHit {
                        path: Some(doc.path.clone()),
                        doc_title: doc.title.clone(),
                        page_one_based: hl.page_one_based,
                        quote: hl.quote.clone(),
                        note: hl.note.clone(),
                    });
                }
            }
        }
        hits
    }
}

pub fn matches_highlight(doc_title: &str, hl: &IndexedHighlight, tokens: &[String]) -> bool {
    let title_l = doc_title.to_ascii_lowercase();
    let quote_l = hl.quote.to_ascii_lowercase();
    let note_l = hl.note.to_ascii_lowercase();
    let page_l = format!("p.{}", hl.page_one_based);
    tokens
        .iter()
        .all(|tok| quote_l.contains(tok) || note_l.contains(tok) || title_l.contains(tok) || page_l.contains(tok))
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_store_persists_dedups_and_searches_highlights() {
        let mut store = LibraryStore::default();
        store.upsert_entry(RecentDocEntry {
            path: PathBuf::from("/tmp/paper_a.pdf"),
            title: "Paper A".to_string(),
            last_opened_secs: 100,
            page_count: 10,
            last_page: 2,
            highlights: vec![IndexedHighlight {
                page_one_based: 3,
                quote: "Normalized coordinates stay invariant across panes".to_string(),
                note: "Key theorem".to_string(),
            }],
        });
        store.upsert_entry(RecentDocEntry {
            path: PathBuf::from("/tmp/paper_b.pdf"),
            title: "Paper B".to_string(),
            last_opened_secs: 200,
            page_count: 8,
            last_page: 1,
            highlights: vec![IndexedHighlight {
                page_one_based: 2,
                quote: "GPU rasterization pipeline".to_string(),
                note: "Fast Metal upload".to_string(),
            }],
        });
        // Updating existing Paper A bumps it to newest
        store.upsert_entry(RecentDocEntry {
            path: PathBuf::from("/tmp/paper_a.pdf"),
            title: "Paper A".to_string(),
            last_opened_secs: 300,
            page_count: 10,
            last_page: 4,
            highlights: vec![IndexedHighlight {
                page_one_based: 3,
                quote: "Normalized coordinates stay invariant across panes".to_string(),
                note: "Key theorem".to_string(),
            }],
        });

        let recent = store.filter_recent("");
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].title, "Paper A");
        assert_eq!(recent[1].title, "Paper B");

        assert_eq!(store.filter_recent("paper b").len(), 1);
        let hits = store.search_highlights("invariant theorem");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_title, "Paper A");
        assert_eq!(hits[0].page_one_based, 3);

        let dir = std::env::temp_dir().join(format!("pinleaf-lib-test-{}", std::process::id()));
        let lib_file = dir.join("library.json");
        store.save_to_path(&lib_file).unwrap();
        let reloaded = LibraryStore::load_from_path(&lib_file);
        assert_eq!(reloaded, store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_config_parses_json_preferences_and_keybindings() {
        let dir = std::env::temp_dir().join(format!("pinleaf-cfg-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_file = dir.join("config.json");
        std::fs::write(
            &cfg_file,
            r#"{
              "default_color_mode": "Sepia",
              "auto_reload": false,
              "keybindings": { "open_search": "secondary-/" }
            }"#,
        )
        .unwrap();
        let cfg = UserConfig::load_from_path(&cfg_file);
        assert_eq!(cfg.default_color_mode, Some(ColorMode::Sepia));
        assert_eq!(cfg.auto_reload, Some(false));
        assert_eq!(cfg.keybindings.get("open_search").map(String::as_str), Some("secondary-/"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
