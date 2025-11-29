pub fn decode_hex_folder_name(hex: &str) -> Option<String> {
    let clean: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();

    if clean.len() % 2 != 0 {
        return None;
    }

    let bytes: Result<Vec<u8>, _> = (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16))
        .collect();

    bytes.ok().and_then(|b| String::from_utf8(b).ok())
}
