use regex::Regex;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ManifestMetadata {
    pub display_name: Option<String>,
    pub package_name: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub categories: Vec<String>,
    pub compatible_versions: Vec<String>,
}

pub fn read_plain_text_lossy(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;

    match String::from_utf8(bytes) {
        Ok(content) => Ok(content),
        Err(error) => Ok(String::from_utf8_lossy(&error.into_bytes()).into_owned()),
    }
}

pub fn parse_manifest_text(text: &str) -> ManifestMetadata {
    let display_name = capture_manifest_value(text, "display_name")
        .or_else(|| capture_manifest_value(text, "name"))
        .or_else(|| capture_manifest_value(text, "package_name"));
    let package_name = capture_manifest_value(text, "package_name")
        .or_else(|| capture_manifest_value(text, "name"));
    let version = capture_manifest_value(text, "version");
    let author = capture_manifest_value(text, "author");
    let description = capture_manifest_value(text, "description");

    let categories = capture_manifest_list(text, "category")
        .into_iter()
        .chain(capture_manifest_list(text, "categories"))
        .collect::<Vec<_>>();
    let compatible_versions = capture_manifest_list(text, "compatible_versions");

    ManifestMetadata {
        display_name,
        package_name,
        version,
        author,
        description,
        categories,
        compatible_versions,
    }
}

fn capture_manifest_value(text: &str, key: &str) -> Option<String> {
    let quoted = Regex::new(&format!(r#"{key}\s*:\s*"([^"]+)""#)).ok()?;
    if let Some(value) = quoted.captures(text).and_then(|capture| capture.get(1)) {
        return Some(value.as_str().trim().to_string());
    }

    let bare = Regex::new(&format!(r#"{key}\s*:\s*([^\r\n]+)"#)).ok()?;
    bare.captures(text)
        .and_then(|capture| capture.get(1))
        .map(|value| {
            value
                .as_str()
                .trim()
                .trim_matches('"')
                .trim_end_matches(';')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn capture_manifest_list(text: &str, key: &str) -> Vec<String> {
    let Some(regex) = Regex::new(&format!(r#"{key}(?:\[\d+\])?\s*:\s*"([^"]+)""#)).ok() else {
        return Vec::new();
    };

    regex
        .captures_iter(text)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().trim().to_string()))
        .filter(|value| !value.is_empty())
        .collect()
}
