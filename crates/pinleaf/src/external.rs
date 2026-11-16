use std::process::Command;

use pinleaf_engine::SyncTexSourceLoc;
use regex::Regex;

/// Converts selected text into an academic DOI landing URL (if a DOI is detected)
/// or a Google Scholar search URL.
pub fn build_scholar_or_doi_url(selection: &str) -> Option<String> {
    let cleaned: String = selection.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return None;
    }

    let doi_re = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Za-z0-9]+").ok()?;
    if let Some(m) = doi_re.find(&cleaned) {
        let doi = m
            .as_str()
            .trim_end_matches(['.', ',', ';', ':', ')', ']', '"', '\'']);
        if !doi.is_empty() {
            return Some(format!("https://doi.org/{doi}"));
        }
    }

    Some(format!(
        "https://scholar.google.com/scholar?q={}",
        urlencoding::encode(&cleaned)
    ))
}

/// Extracts an academic DOI token if present in `text`.
pub fn extract_first_doi(text: &str) -> String {
    let Ok(doi_re) = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Za-z0-9]+") else {
        return String::new();
    };
    doi_re
        .find(text)
        .map(|m| {
            m.as_str()
                .trim_end_matches(['.', ',', ';', ':', ')', ']', '"', '\''])
                .to_string()
        })
        .unwrap_or_default()
}

/// Opens a URL using the operating system's default browser.
pub fn open_external_url(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWorkspace;
        use objc2_foundation::{NSString, NSURL};

        let ns_str = NSString::from_str(url);
        if let Some(ns_url) = NSURL::URLWithString(&ns_str) {
            return NSWorkspace::sharedWorkspace().openURL(&ns_url);
        }
        false
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .is_ok()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Command::new("xdg-open").arg(url).spawn().is_ok()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExtensionContext {
    pub file_path: String,
    pub page_one_based: u32,
    pub selected_text: String,
    pub line_text: String,
    pub doi: String,
}

pub fn shell_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

pub fn expand_command_template(template: &str, ctx: &ExtensionContext) -> String {
    let page_str = ctx.page_one_based.to_string();
    let q_file = shell_single_quote(&ctx.file_path);
    let q_sel = shell_single_quote(&ctx.selected_text);
    let q_line = shell_single_quote(&ctx.line_text);
    let q_doi = shell_single_quote(&ctx.doi);

    template
        .replace("%{file_path}", &q_file)
        .replace("{file_path}", &q_file)
        .replace("%{selected_text}", &q_sel)
        .replace("{selected_text}", &q_sel)
        .replace("%{page_number}", &page_str)
        .replace("{page}", &page_str)
        .replace("%{line_text}", &q_line)
        .replace("{line_text}", &q_line)
        .replace("%{doi}", &q_doi)
        .replace("{doi}", &q_doi)
}

pub fn run_shell_extension_blocking(template: &str, ctx: &ExtensionContext) -> Result<String, String> {
    let expanded = expand_command_template(template, ctx);
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(&expanded)
        .env("PINLEAF_FILE_PATH", &ctx.file_path)
        .env("PINLEAF_PAGE", ctx.page_one_based.to_string())
        .env("PINLEAF_SELECTED_TEXT", &ctx.selected_text)
        .env("PINLEAF_LINE_TEXT", &ctx.line_text)
        .env("PINLEAF_DOI", &ctx.doi)
        .output()
        .map_err(|e| format!("Failed to spawn shell extension: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        if stdout.is_empty() {
            Ok("Extension finished".to_string())
        } else {
            Ok(stdout.chars().take(120).collect())
        }
    } else if !stderr.is_empty() {
        Err(format!("Extension failed: {}", stderr.chars().take(120).collect::<String>()))
    } else {
        Err(format!("Extension exited with status {}", output.status))
    }
}

pub fn run_synctex_inverse_editor(
    loc: &SyncTexSourceLoc,
    custom_cmd: Option<&str>,
) -> (String, bool) {
    let file_str = loc.file.to_string_lossy().to_string();
    let line_str = loc.line.to_string();
    let col_str = loc.column.max(1).to_string();

    if let Some(tmpl) = custom_cmd.filter(|s| !s.trim().is_empty()) {
        let cmd = tmpl
            .replace("{file}", &shell_single_quote(&file_str))
            .replace("{line}", &line_str)
            .replace("{col}", &col_str);
        return match Command::new("/bin/sh").arg("-c").arg(&cmd).spawn() {
            Ok(_) => (
                format!(
                    "SyncTeX -> {}:{}",
                    loc.file.file_name().and_then(|n| n.to_str()).unwrap_or(&file_str),
                    loc.line
                ),
                false,
            ),
            Err(err) => (format!("SyncTeX inverse command failed: {err}"), true),
        };
    }

    let loc_arg = format!("{file_str}:{line_str}:{col_str}");
    for editor in ["code", "cursor", "subl"] {
        if Command::new(editor).args(["-g", &loc_arg]).spawn().is_ok() {
            return (
                format!(
                    "SyncTeX ({editor}) -> {}:{}",
                    loc.file.file_name().and_then(|n| n.to_str()).unwrap_or(&file_str),
                    loc.line
                ),
                false,
            );
        }
    }

    (
        format!(
            "SyncTeX match: {}:{} (configure inverse_search_command to open editor)",
            loc.file.file_name().and_then(|n| n.to_str()).unwrap_or(&file_str),
            loc.line
        ),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_doi_and_scholar_urls_cleanly() {
        assert_eq!(build_scholar_or_doi_url("   \n\t  "), None);
        assert_eq!(
            build_scholar_or_doi_url("doi: 10.1145/3597503.3623310."),
            Some("https://doi.org/10.1145/3597503.3623310".to_string())
        );
        assert_eq!(
            build_scholar_or_doi_url("  Freeze-and-Branch   Multi-View\nReading "),
            Some(
                "https://scholar.google.com/scholar?q=Freeze-and-Branch%20Multi-View%20Reading"
                    .to_string()
            )
        );
    }

    #[test]
    fn expand_command_template_quotes_shell_metacharacters() {
        let ctx = ExtensionContext {
            file_path: "/tmp/My Paper's (v2).pdf".to_string(),
            page_one_based: 4,
            selected_text: "hello $(rm -rf) 'world'".to_string(),
            line_text: "Theorem 1".to_string(),
            doi: "10.1145/1234.5678".to_string(),
        };
        let cmd = expand_command_template(
            "echo {page} %{selected_text} {file_path} %{doi}",
            &ctx,
        );
        assert_eq!(
            cmd,
            r#"echo 4 'hello $(rm -rf) '"'"'world'"'"'' '/tmp/My Paper'"'"'s (v2).pdf' '10.1145/1234.5678'"#
        );
    }
}
