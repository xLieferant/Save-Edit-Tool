use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::shared::decrypt::decode_text_bytes;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PowertrainBuildSummary {
    pub output_path: String,
    pub engine_count: usize,
    pub transmission_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct PowertrainCatalogFile {
    schema_version: u32,
    game: String,
    game_version: String,
    generated_at: String,
    sources: Vec<String>,
    engines: Vec<PowertrainEngineFile>,
    transmissions: Vec<PowertrainTransmissionFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct PowertrainEngineFile {
    id: String,
    data_path: String,
    brand: String,
    truck_model: String,
    name: String,
    #[serde(rename = "type")]
    engine_type: String,
    torque_nm: Option<f64>,
    power: Option<f64>,
    rpm_idle: Option<f64>,
    rpm_limit: Option<f64>,
    official: bool,
    source_archive: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct PowertrainTransmissionFile {
    id: String,
    data_path: String,
    brand: String,
    truck_model: String,
    name: String,
    gears_forward: Option<u32>,
    ratios_forward: Vec<f64>,
    ratios_reverse: Vec<f64>,
    differential_ratio: Option<f64>,
    retarder_steps: Option<u32>,
    official: bool,
    source_archive: String,
}

pub fn build_powertrain_catalog(
    repo_root: &Path,
    source_path: &Path,
    game: &str,
    game_version: &str,
) -> Result<PowertrainBuildSummary, String> {
    validate_official_source_path(source_path)?;
    let files = load_powertrain_files(source_path)?;
    let mut warnings = Vec::new();
    let source_label = source_path.display().to_string();
    let source_archive = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("official_source")
        .to_string();
    let mut engines = Vec::new();
    let mut transmissions = Vec::new();

    for path in files.keys().cloned().collect::<Vec<_>>() {
        let normalized = normalize_rel_path(&path);
        if !normalized.ends_with(".sii") {
            continue;
        }
        if !(normalized.contains("/engine/") || normalized.contains("/transmission/")) {
            continue;
        }
        let resolved = match resolve_includes(&normalized, &files) {
            Ok(content) => content,
            Err(error) => {
                warnings.push(error);
                continue;
            }
        };
        let Some((brand, truck_model)) = brand_model_from_powertrain_path(&normalized) else {
            warnings.push(format!("powertrain_path_unrecognized:{}", normalized));
            continue;
        };
        if normalized.contains("/engine/") {
            engines.push(parse_engine_file(
                &normalized,
                &resolved,
                &brand,
                &truck_model,
                &source_archive,
            ));
        } else if normalized.contains("/transmission/") {
            transmissions.push(parse_transmission_file(
                &normalized,
                &resolved,
                &brand,
                &truck_model,
                &source_archive,
            ));
        }
    }

    engines.sort_by(|left, right| left.data_path.cmp(&right.data_path));
    transmissions.sort_by(|left, right| left.data_path.cmp(&right.data_path));

    let output_dir = repo_root.join("data").join("vehicle");
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let output_path = output_dir.join(format!("powertrain_catalog.{}.{}.json", game, game_version));
    let catalog = PowertrainCatalogFile {
        schema_version: 1,
        game: game.to_string(),
        game_version: game_version.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        sources: vec![source_label],
        engines,
        transmissions,
    };
    let json = serde_json::to_string_pretty(&catalog).map_err(|error| error.to_string())?;
    fs::write(&output_path, format!("{}\n", json)).map_err(|error| error.to_string())?;

    Ok(PowertrainBuildSummary {
        output_path: output_path.display().to_string(),
        engine_count: catalog.engines.len(),
        transmission_count: catalog.transmissions.len(),
        warnings,
    })
}

pub fn validate_official_source_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("official_source_not_found:{}", path.display()));
    }

    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(|component| component.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for window in components.windows(3) {
        if window[0] == "steamapps" && window[1] == "workshop" && window[2] == "content" {
            return Err("mod_or_workshop_source_rejected".to_string());
        }
    }
    if components
        .iter()
        .any(|component| component == "mod" || component == "workshop")
    {
        return Err("mod_or_workshop_source_rejected".to_string());
    }
    if path.is_file()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| !value.eq_ignore_ascii_case("scs"))
            .unwrap_or(true)
    {
        return Err("official_source_must_be_scs_archive_or_directory".to_string());
    }
    Ok(())
}

fn load_powertrain_files(source_path: &Path) -> Result<HashMap<String, String>, String> {
    if source_path.is_dir() {
        load_powertrain_files_from_dir(source_path)
    } else {
        load_powertrain_files_from_archive(source_path)
    }
}

fn load_powertrain_files_from_dir(source_path: &Path) -> Result<HashMap<String, String>, String> {
    let mut files = HashMap::new();
    for entry in WalkDir::new(source_path).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(source_path)
            .map_err(|error| error.to_string())?;
        let normalized = normalize_rel_path(&relative.display().to_string());
        if !is_powertrain_rel_path(&normalized) {
            continue;
        }
        let content = fs::read_to_string(entry.path()).map_err(|error| error.to_string())?;
        files.insert(normalized, content);
    }
    Ok(files)
}

fn load_powertrain_files_from_archive(
    source_path: &Path,
) -> Result<HashMap<String, String>, String> {
    let file = fs::File::open(source_path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut files = HashMap::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if !entry.is_file() {
            continue;
        }
        let name = normalize_rel_path(entry.name());
        if !is_powertrain_rel_path(&name) {
            continue;
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        let content = decode_text_bytes(&bytes, &name, &[source_path.display().to_string()])?;
        files.insert(name, content);
    }
    Ok(files)
}

fn is_powertrain_rel_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("def/vehicle/truck/")
        && (lower.contains("/engine/") || lower.contains("/transmission/"))
        && (lower.ends_with(".sii") || lower.ends_with(".sui"))
}

fn resolve_includes(path: &str, files: &HashMap<String, String>) -> Result<String, String> {
    let mut stack = Vec::new();
    let mut visited = HashSet::new();
    resolve_includes_inner(path, files, &mut stack, &mut visited)
}

fn resolve_includes_inner(
    path: &str,
    files: &HashMap<String, String>,
    stack: &mut Vec<String>,
    visited: &mut HashSet<String>,
) -> Result<String, String> {
    let normalized = normalize_rel_path(path);
    if stack.iter().any(|item| item == &normalized) {
        stack.push(normalized.clone());
        return Err(format!("include_cycle_detected:{}", stack.join("->")));
    }
    let content = files
        .get(&normalized)
        .ok_or_else(|| format!("include_missing:{}", normalized))?;
    if !visited.insert(normalized.clone()) {
        return Ok(content.clone());
    }
    stack.push(normalized.clone());
    let include_re = Regex::new(r#"@include\s+"([^"]+)""#).expect("valid include regex");
    let mut output = String::new();
    for line in content.lines() {
        if let Some(captures) = include_re.captures(line.trim()) {
            let include_path = captures.get(1).map(|value| value.as_str()).unwrap_or("");
            let resolved = resolve_include_path(&normalized, include_path);
            output.push_str(&resolve_includes_inner(&resolved, files, stack, visited)?);
            output.push('\n');
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    stack.pop();
    Ok(output)
}

fn resolve_include_path(current_path: &str, include_path: &str) -> String {
    let include = include_path.replace('\\', "/");
    if include.starts_with('/') {
        return normalize_rel_path(include.trim_start_matches('/'));
    }
    let parent = current_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    normalize_rel_path(&format!("{}/{}", parent, include))
}

fn brand_model_from_powertrain_path(path: &str) -> Option<(String, String)> {
    let parts = path.split('/').collect::<Vec<_>>();
    let family_index = parts
        .iter()
        .position(|part| *part == "truck")
        .and_then(|index| parts.get(index + 1).copied())?;
    let mut family_parts = family_index.split('.');
    let brand = family_parts.next()?.to_string();
    let model = family_parts.collect::<Vec<_>>().join(".");
    Some((brand, model))
}

fn parse_engine_file(
    path: &str,
    content: &str,
    brand: &str,
    truck_model: &str,
    source_archive: &str,
) -> PowertrainEngineFile {
    PowertrainEngineFile {
        id: unit_id_or_file_stem(path, content),
        data_path: format!("/{}", path),
        brand: brand.to_string(),
        truck_model: truck_model.to_string(),
        name: field_value(content, "name").unwrap_or_else(|| file_stem(path)),
        engine_type: field_value(content, "type").unwrap_or_else(|| "diesel".to_string()),
        torque_nm: field_f64(content, "torque"),
        power: field_f64(content, "power"),
        rpm_idle: field_f64(content, "rpm_idle"),
        rpm_limit: field_f64(content, "rpm_limit"),
        official: true,
        source_archive: source_archive.to_string(),
    }
}

fn parse_transmission_file(
    path: &str,
    content: &str,
    brand: &str,
    truck_model: &str,
    source_archive: &str,
) -> PowertrainTransmissionFile {
    let ratios_forward = array_f64(content, "ratios_forward");
    let ratios_reverse = array_f64(content, "ratios_reverse");
    PowertrainTransmissionFile {
        id: unit_id_or_file_stem(path, content),
        data_path: format!("/{}", path),
        brand: brand.to_string(),
        truck_model: truck_model.to_string(),
        name: field_value(content, "name").unwrap_or_else(|| file_stem(path)),
        gears_forward: field_u32(content, "gears_forward")
            .or_else(|| Some(ratios_forward.len() as u32).filter(|value| *value > 0)),
        ratios_forward,
        ratios_reverse,
        differential_ratio: field_f64(content, "differential_ratio"),
        retarder_steps: field_u32(content, "retarder"),
        official: true,
        source_archive: source_archive.to_string(),
    }
}

fn unit_id_or_file_stem(path: &str, content: &str) -> String {
    Regex::new(r"(?m)^\s*[A-Za-z0-9_]+\s*:\s*([^\s{]+)")
        .ok()
        .and_then(|regex| regex.captures(content))
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
        .unwrap_or_else(|| file_stem(path))
}

fn field_value(content: &str, field: &str) -> Option<String> {
    let prefix = format!("{}:", field);
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(&prefix) {
            return None;
        }
        trimmed
            .split_once(':')
            .map(|(_, value)| clean_value(value))
            .filter(|value| !value.is_empty())
    })
}

fn field_f64(content: &str, field: &str) -> Option<f64> {
    field_value(content, field).and_then(|value| value.parse::<f64>().ok())
}

fn field_u32(content: &str, field: &str) -> Option<u32> {
    field_value(content, field).and_then(|value| value.parse::<u32>().ok())
}

fn array_f64(content: &str, field: &str) -> Vec<f64> {
    let prefix = format!("{}[", field);
    let mut values = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(&prefix) {
            continue;
        }
        let Some(index) = trimmed
            .split_once('[')
            .and_then(|(_, tail)| tail.split_once(']'))
            .and_then(|(value, _)| value.parse::<usize>().ok())
        else {
            continue;
        };
        let Some(value) = trimmed
            .split_once(':')
            .map(|(_, value)| clean_value(value))
            .and_then(|value| value.parse::<f64>().ok())
        else {
            continue;
        };
        values.insert(index, value);
    }
    values.into_values().collect()
}

fn clean_value(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(',')
        .trim_matches('"')
        .trim()
        .to_string()
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn normalize_rel_path(path: &str) -> String {
    let mut parts = Vec::new();
    let normalized = path.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::{build_powertrain_catalog, validate_official_source_path};
    use std::fs;

    #[test]
    fn catalog_rejects_mod_or_workshop_source() {
        let root =
            std::env::temp_dir().join(format!("ets2_powertrain_mod_reject_{}", std::process::id()));
        let path = root.join("Euro Truck Simulator 2").join("mod");
        fs::create_dir_all(&path).unwrap();
        let error = validate_official_source_path(&path).unwrap_err();
        assert_eq!(error, "mod_or_workshop_source_rejected");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn builder_detects_include_cycles() {
        let root =
            std::env::temp_dir().join(format!("ets2_powertrain_cycle_{}", std::process::id()));
        let source = root.join("official");
        let engine_dir = source.join("def/vehicle/truck/scania.s_2016/engine");
        fs::create_dir_all(&engine_dir).unwrap();
        fs::write(
            engine_dir.join("a.sii"),
            "SiiNunit\n{\n@include \"b.sui\"\n}\n",
        )
        .unwrap();
        fs::write(engine_dir.join("b.sui"), "@include \"a.sii\"\n").unwrap();

        let result = build_powertrain_catalog(&root, &source, "ets2", "test").unwrap();
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("include_cycle_detected"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn builder_reads_transmission_differential_ratio() {
        let root =
            std::env::temp_dir().join(format!("ets2_powertrain_diff_{}", std::process::id()));
        let source = root.join("official");
        let transmission_dir = source.join("def/vehicle/truck/scania.s_2016/transmission");
        fs::create_dir_all(&transmission_dir).unwrap();
        fs::write(
            transmission_dir.join("g33.sii"),
            r#"SiiNunit
{
accessory_transmission_data : transmission.g33 {
 name: "G33"
 differential_ratio: 2.59
 ratios_forward[0]: 14.94
}
}
"#,
        )
        .unwrap();

        let result = build_powertrain_catalog(&root, &source, "ets2", "test").unwrap();
        assert_eq!(result.transmission_count, 1);
        let output = fs::read_to_string(result.output_path).unwrap();
        assert!(output.contains("\"differentialRatio\": 2.59"));
        let _ = fs::remove_dir_all(root);
    }
}
