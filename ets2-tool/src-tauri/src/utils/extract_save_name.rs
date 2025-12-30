use regex::Regex;

pub fn extract_save_name(content: &str) -> Option<String> {
    // name: test
    // name: "Langer Test"
    let re = Regex::new(r#"name:\s*(?:"([^"]+)"|([^\s]+))"#).ok()?;

    re.captures(content).and_then(|c| {
        c.get(1)
            .or_else(|| c.get(2))
            .map(|m| m.as_str().to_string())
    })
}
