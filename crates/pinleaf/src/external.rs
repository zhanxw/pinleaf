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
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .is_ok()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .is_ok()
    }
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
            Some("https://scholar.google.com/scholar?q=Freeze-and-Branch%20Multi-View%20Reading".to_string())
        );
    }
}
