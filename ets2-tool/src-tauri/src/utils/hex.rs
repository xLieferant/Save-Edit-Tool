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
