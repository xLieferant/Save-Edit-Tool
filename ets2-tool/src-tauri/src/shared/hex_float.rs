use crate::dev_log;

/// Konvertiert einen SII-Wert wie "&3d086363" → f32 (IEEE 754)
pub fn hex_to_float(token: &str) -> Result<f32, String> {
    dev_log!("hex_to_float: Eingabe = {}", token);

    let cleaned = token
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("0x");

    let bits = u32::from_str_radix(cleaned, 16)
        .map_err(|e| format!("Ungültiger Hexwert '{}': {}", token, e))?;

    let value = f32::from_bits(bits);

    dev_log!("hex_to_float: {} -> {}", token, value);

    Ok(value)
}

/// Konvertiert einen f32 (z. B. 0.83 oder 1.0) → SII-Hex-Format "&3f4ccccd"
pub fn float_to_hex(value: f32) -> String {
    dev_log!("float_to_hex: Eingabe = {}", value);

    let bits = value.to_bits();
    let hex = format!("&{:08x}", bits);

    dev_log!("float_to_hex: {} -> {}", value, hex);

    hex
}

/// Komfortfunktion (für später): erkennt automatisch, ob der Input hex oder float ist
/// Beispiele:
///   "0.83" → 0.83
///   "&3d086363" → 0.0332...
pub fn parse_value_auto(input: &str) -> Result<f32, String> {
    let trimmed = input.trim();

    // Fall 1: Hex-Format
    if trimmed.starts_with('&') || trimmed.starts_with("0x") {
        return hex_to_float(trimmed);
    }

    // Fall 2: normaler Float (z. B. aus UI)
    match trimmed.replace(',', ".").parse::<f32>() {
        Ok(v) => {
            dev_log!("parse_value_auto: Float erkannt: {}", v);
            Ok(v)
        }
        Err(_) => Err(format!(
            "Konnte '{}' nicht als Hex oder Float interpretieren",
            input
        )),
    }
}

pub fn decode_hex_folder_name(hex: &str) -> Option<String> {
    let hex_clean: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex_clean.len() % 2 != 0 || hex_clean.is_empty() {
        return None;
    }

    let bytes_res: Result<Vec<u8>, _> = (0..hex_clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_clean[i..i + 2], 16))
        .collect();

    bytes_res.ok().and_then(|b| String::from_utf8(b).ok())
}

pub fn text_to_hex(text: &str) -> String {
    text.as_bytes().iter().map(|b| format!("{:02X}", b)).collect()
}
