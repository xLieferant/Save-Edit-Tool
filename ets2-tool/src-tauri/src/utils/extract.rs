use regex::Regex;

pub fn extract_profile_name(text: &str) -> Option<String> {
    let re = Regex::new(r#"(?i)profile_name\s*:\s*"?(?P<n>[^"\n\r]+)"?"#).unwrap();
    re.captures(text)
        .and_then(|c| c.name("n"))
        .map(|m| m.as_str().trim().to_string())
}
