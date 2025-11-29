use regex::Regex;

/// Extrahiert `profile_name` aus profile.sii.
/// Beispiel-matching: profile_name: "Name"  oder profile_name: Name
pub fn extract_profile_name(text: &str) -> Option<String> {
    let re = Regex::new(r#"(?i)profile_name\s*:\s*"?(?P<name>[^"\r\n]+)"?"#).ok()?;
    re.captures(text)
        .and_then(|c| c.name("name"))
        .map(|m| m.as_str().trim().to_string())
}

/// Extrahiert numerischen Wert für einen Key (z.B. money_account)
pub fn extract_value(text: &str, key: &str) -> Option<i64> {
    let pattern = format!(r#"(?m)^\s*{}\s*:\s*(-?\d+)"#, regex::escape(key));
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(text)?;
    caps.get(1)?.as_str().parse::<i64>().ok()
}
