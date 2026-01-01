use regex::Regex;

pub fn extract_profile_name(text: &str) -> Option<String> {
    let re = Regex::new(r#"(?i)profile_name\s*:\s*"?(?P<name>[^"\r\n]+)"?"#).unwrap();
    re.captures(text).map(|c| c[1].trim().to_string())
}
