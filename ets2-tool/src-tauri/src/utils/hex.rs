/// Dekodiert hex-codierte Ordnernamen (ETS2 nutzt oft hex-IDs)
pub fn decode_hex_folder_name(hex: &str) -> Option<String> {
    let clean: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.is_empty() || (clean.len() % 2 != 0) {
        return None;
    }

    let bytes_res: Result<Vec<u8>, _> = (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16))
        .collect();

    match bytes_res {
        Ok(bytes) => String::from_utf8(bytes).ok(),
        Err(_) => None,
    }
}
