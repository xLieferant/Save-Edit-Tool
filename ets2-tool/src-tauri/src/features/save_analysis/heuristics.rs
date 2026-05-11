#[derive(Debug, Clone)]
pub struct HeuristicMatch {
    pub category: &'static str,
    pub title: &'static str,
    pub explanation: &'static str,
    pub severity: &'static str,
    pub confidence: &'static str,
    pub base_score: u32,
    pub preferred_mod_categories: &'static [&'static str],
}

pub fn match_log_line(line: &str) -> Option<HeuristicMatch> {
    let normalized = line.to_ascii_lowercase();
    let has_error = normalized.contains("error");
    let has_warning = normalized.contains("warning");
    let severity = if has_error { "ERROR" } else if has_warning { "WARNING" } else { "INFO" };

    let rules: [(&str, HeuristicMatch); 15] = [
        (
            "missing accessory",
            HeuristicMatch {
                category: "Accessory Reference",
                title: "Missing accessory reference",
                explanation: "The log references an accessory that is no longer available. This often happens after removing or updating truck, trailer or accessory mods.",
                severity: "ERROR",
                confidence: "Likely",
                base_score: 34,
                preferred_mod_categories: &["Accessory Mod", "Truck Mod", "Trailer Mod"],
            },
        ),
        (
            "incorrect rear wheel definition",
            HeuristicMatch {
                category: "Wheel Definition",
                title: "Invalid rear wheel definition",
                explanation: "The game detected an invalid wheel definition. Trailer, truck and wheel mods become more suspicious when this appears shortly before a crash.",
                severity: "ERROR",
                confidence: "Likely",
                base_score: 32,
                preferred_mod_categories: &["Trailer Mod", "Accessory Mod", "Truck Mod"],
            },
        ),
        (
            "failed to open",
            HeuristicMatch {
                category: "Missing Resource",
                title: "Resource failed to open",
                explanation: "A file could not be opened. Missing meshes, materials, textures or definitions often point to removed or incompatible mods.",
                severity: "ERROR",
                confidence: "Likely",
                base_score: 30,
                preferred_mod_categories: &["Unknown / Mixed"],
            },
        ),
        (
            "failed to load",
            HeuristicMatch {
                category: "Missing Resource",
                title: "Resource failed to load",
                explanation: "A resource could not be loaded successfully. This usually indicates missing files, broken load order or incompatible packages.",
                severity: "ERROR",
                confidence: "Likely",
                base_score: 28,
                preferred_mod_categories: &["Unknown / Mixed"],
            },
        ),
        (
            ".sii",
            HeuristicMatch {
                category: "Definition File",
                title: "Definition file issue",
                explanation: "A `.sii` definition file appears inside a warning or error. That often means a missing definition or an invalid dependency chain.",
                severity,
                confidence: "Possible",
                base_score: 18,
                preferred_mod_categories: &["Unknown / Mixed"],
            },
        ),
        (
            ".pmd",
            HeuristicMatch {
                category: "Model Resource",
                title: "Model file issue",
                explanation: "A `.pmd` file is involved in the failure. Model resources are commonly affected by truck, trailer and accessory mods.",
                severity,
                confidence: "Possible",
                base_score: 18,
                preferred_mod_categories: &["Truck Mod", "Trailer Mod", "Accessory Mod"],
            },
        ),
        (
            ".pmg",
            HeuristicMatch {
                category: "Model Resource",
                title: "Geometry file issue",
                explanation: "A `.pmg` file is involved in the failure. Geometry resources often break when a mod update is incomplete or assets were removed.",
                severity,
                confidence: "Possible",
                base_score: 18,
                preferred_mod_categories: &["Truck Mod", "Trailer Mod", "Accessory Mod"],
            },
        ),
        (
            ".mat",
            HeuristicMatch {
                category: "Material Resource",
                title: "Material file issue",
                explanation: "A `.mat` file appears in the error chain. Material issues usually point to missing textures, UI mods or incompatible visual assets.",
                severity,
                confidence: "Possible",
                base_score: 16,
                preferred_mod_categories: &["Accessory Mod", "UI / Route Advisor Mod", "Truck Mod"],
            },
        ),
        (
            ".tobj",
            HeuristicMatch {
                category: "Texture Object",
                title: "Texture object issue",
                explanation: "A `.tobj` texture object could not be resolved. UI and visual mods become more suspicious when this happens.",
                severity,
                confidence: "Possible",
                base_score: 16,
                preferred_mod_categories: &["UI / Route Advisor Mod", "Accessory Mod"],
            },
        ),
        (
            "cargo market",
            HeuristicMatch {
                category: "Cargo Market",
                title: "Cargo market related failure",
                explanation: "The failure happened around cargo market logic. Cargo, trailer, economy and map mods should be prioritised.",
                severity,
                confidence: "Possible",
                base_score: 22,
                preferred_mod_categories: &["Cargo Mod", "Trailer Mod", "Map Mod"],
            },
        ),
        (
            "route advisor",
            HeuristicMatch {
                category: "UI / Route Advisor",
                title: "Route advisor related failure",
                explanation: "The route advisor or HUD appears in the error chain. UI or route advisor mods become more suspicious.",
                severity,
                confidence: "Possible",
                base_score: 22,
                preferred_mod_categories: &["UI / Route Advisor Mod"],
            },
        ),
        (
            "prefab",
            HeuristicMatch {
                category: "Prefab / Map",
                title: "Prefab reference issue",
                explanation: "A prefab reference is failing. Map mods or broken load order are common root causes for this pattern.",
                severity,
                confidence: "Likely",
                base_score: 28,
                preferred_mod_categories: &["Map Mod"],
            },
        ),
        (
            "dealer",
            HeuristicMatch {
                category: "Dealer / Vehicle Browser",
                title: "Dealer related issue",
                explanation: "The crash path includes dealer logic. Truck, accessory and UI mods become more suspicious when browsing or previewing vehicles.",
                severity,
                confidence: "Possible",
                base_score: 20,
                preferred_mod_categories: &["Truck Mod", "Accessory Mod", "UI / Route Advisor Mod"],
            },
        ),
        (
            "traffic",
            HeuristicMatch {
                category: "Traffic",
                title: "Traffic related issue",
                explanation: "Traffic systems appear in the failure chain. Traffic packs and AI traffic mods should be prioritised.",
                severity,
                confidence: "Possible",
                base_score: 20,
                preferred_mod_categories: &["Traffic Mod"],
            },
        ),
        (
            "map",
            HeuristicMatch {
                category: "Map",
                title: "Map related issue",
                explanation: "The failure references map-related content. Map mods and load order conflicts are typical causes for this pattern.",
                severity,
                confidence: "Possible",
                base_score: 20,
                preferred_mod_categories: &["Map Mod"],
            },
        ),
    ];

    for (needle, rule) in rules {
        if normalized.contains(needle) {
            return Some(rule);
        }
    }

    if has_error && normalized.contains("missing") {
        return Some(HeuristicMatch {
            category: "Missing Resource",
            title: "Missing resource",
            explanation: "A resource is missing according to the log. That often points to removed, outdated or partially broken mods.",
            severity: "ERROR",
            confidence: "Possible",
            base_score: 18,
            preferred_mod_categories: &["Unknown / Mixed"],
        });
    }

    if has_warning && (normalized.contains("accessory") || normalized.contains("cargo") || normalized.contains("trailer")) {
        return Some(HeuristicMatch {
            category: "Save / Asset Warning",
            title: "Asset warning",
            explanation: "The log contains a warning about assets that are commonly supplied by mods. It may be harmless, but it should be reviewed together with the save state.",
            severity: "WARNING",
            confidence: "Possible",
            base_score: 10,
            preferred_mod_categories: &["Unknown / Mixed"],
        });
    }

    None
}

pub fn classify_mod_category(text: &str) -> String {
    let normalized = text.to_ascii_lowercase();

    if contains_any(&normalized, &["route advisor", "ui", "hud", "dashboard", "gps"]) {
        return "UI / Route Advisor Mod".to_string();
    }
    if contains_any(&normalized, &["traffic", "ai traffic", "jazzycat"]) {
        return "Traffic Mod".to_string();
    }
    if contains_any(&normalized, &["cargo", "economy", "freight", "market"]) {
        return "Cargo Mod".to_string();
    }
    if contains_any(&normalized, &["prefab", "map", "promods", "road", "city", "depot"]) {
        return "Map Mod".to_string();
    }
    if contains_any(&normalized, &["trailer", "krone", "schmitz"]) {
        return "Trailer Mod".to_string();
    }
    if contains_any(&normalized, &["accessory", "interior", "wheel", "paint", "tuning"]) {
        return "Accessory Mod".to_string();
    }
    if contains_any(
        &normalized,
        &[
            "truck",
            "scania",
            "volvo",
            "daf",
            "man",
            "mercedes",
            "renault",
            "iveco",
            "kenworth",
            "peterbilt",
        ],
    ) {
        return "Truck Mod".to_string();
    }

    "Unknown / Mixed".to_string()
}

pub fn category_bias_matches(mod_category: &str, preferred: &[&str]) -> bool {
    if preferred.is_empty() {
        return false;
    }
    preferred
        .iter()
        .any(|value| value.eq_ignore_ascii_case(mod_category))
}

pub fn confidence_from_score(score: u32, confirmed: bool) -> String {
    if confirmed {
        return "Confirmed".to_string();
    }
    if score >= 60 {
        return "Likely".to_string();
    }
    if score >= 25 {
        return "Possible".to_string();
    }
    "Unknown".to_string()
}

pub fn suspicion_level_from_score(score: u32) -> String {
    if score >= 70 {
        return "High suspicion".to_string();
    }
    if score >= 35 {
        return "Medium suspicion".to_string();
    }
    "Low suspicion".to_string()
}

pub fn save_health_from_score(score: u32) -> String {
    if score >= 70 {
        return "Red".to_string();
    }
    if score >= 35 {
        return "Yellow".to_string();
    }
    "Green".to_string()
}

pub fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}
