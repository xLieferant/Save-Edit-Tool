use regex::Regex;
use serde::Serialize;
use std::fmt;

pub const MAX_LICENSE_PLATE_CHARS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicensePlateError {
    Empty,
    TooLong,
    InvalidText,
    InvalidColor,
    MalformedFormat,
    UnsupportedFormat,
    UnsupportedColor,
}

impl LicensePlateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty => "license_plate_empty",
            Self::TooLong => "license_plate_too_long",
            Self::InvalidText => "license_plate_invalid",
            Self::InvalidColor => "license_plate_color_invalid",
            Self::MalformedFormat => "license_plate_malformed",
            Self::UnsupportedFormat => "license_plate_unsupported",
            Self::UnsupportedColor => "license_plate_color_unsupported",
        }
    }
}

impl fmt::Display for LicensePlateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum PlateToken {
    Text(String),
    Offset {
        hshift: Option<String>,
        vshift: Option<String>,
    },
    Image {
        src: String,
        color: Option<String>,
    },
    RawMarkup(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholePlateTextStrategy {
    ReplaceFirstSegmentAndClearRemaining,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ParsedLicensePlate {
    pub raw: String,
    pub country: Option<String>,
    pub tokens: Vec<PlateToken>,
    pub content: String,
    pub visible_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Tag,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenSpan {
    kind: TokenKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextSegment {
    start: usize,
    end: usize,
    inner_start: usize,
    inner_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReplacementRange {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RgbColor {
    red: u8,
    green: u8,
    blue: u8,
}

pub fn parse_license_plate(raw_plate: &str) -> Result<ParsedLicensePlate, LicensePlateError> {
    let raw = trim_wrapping_quotes(raw_plate).to_string();
    let (content, suffix) = split_country_suffix(&raw);
    let content = content.to_string();
    let country = suffix
        .filter(|country| !country.is_empty())
        .map(str::to_string);
    let token_spans = tokenize_content(&content)?;
    let tokens = render_tokens(&content, &token_spans);
    let text_segments = editable_text_segments(&content, &token_spans);
    let visible_text = text_segments
        .iter()
        .map(|segment| content[segment.inner_start..segment.inner_end].trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    Ok(ParsedLicensePlate {
        raw,
        country,
        tokens,
        content,
        visible_text,
    })
}

pub fn serialize_license_plate(parsed: &ParsedLicensePlate) -> String {
    parsed.raw.clone()
}

pub fn serialize_without_changes(raw_plate: &str) -> Result<String, LicensePlateError> {
    let parsed = parse_license_plate(raw_plate)?;
    Ok(serialize_license_plate(&parsed))
}

pub fn license_plate_display_text(raw_plate: &str) -> String {
    parse_license_plate(raw_plate)
        .map(|parsed| parsed.visible_text)
        .unwrap_or_else(|_| fallback_display_text(raw_plate))
}

pub fn validate_license_plate_text(new_text: &str) -> Result<String, LicensePlateError> {
    let trimmed = new_text.trim();
    if trimmed.is_empty() {
        return Err(LicensePlateError::Empty);
    }
    if trimmed.chars().count() > MAX_LICENSE_PLATE_CHARS {
        return Err(LicensePlateError::TooLong);
    }
    if trimmed.chars().any(|character| {
        character.is_control() || matches!(character, '"' | '\\' | '|' | '<' | '>')
    }) {
        return Err(LicensePlateError::InvalidText);
    }

    Ok(trimmed.to_string())
}

pub fn replace_license_plate_text(
    raw_plate: &str,
    new_text: &str,
) -> Result<String, LicensePlateError> {
    replace_whole_plate_text(
        raw_plate,
        new_text,
        WholePlateTextStrategy::ReplaceFirstSegmentAndClearRemaining,
    )
}

pub fn replace_plate_segment(
    raw_plate: &str,
    segment_index: usize,
    new_text: &str,
) -> Result<String, LicensePlateError> {
    let new_text = validate_license_plate_text(new_text)?;
    let raw = trim_wrapping_quotes(raw_plate);
    let (content, suffix) = split_country_suffix(raw);
    let token_spans = tokenize_content(content)?;
    let text_segments = editable_text_segments(content, &token_spans);
    let segment = text_segments
        .get(segment_index)
        .ok_or(LicensePlateError::UnsupportedFormat)?;
    let replaced_content = replace_single_text_segment(content, segment, &new_text);
    Ok(join_country_suffix(&replaced_content, suffix))
}

pub fn replace_whole_plate_text(
    raw_plate: &str,
    new_text: &str,
    strategy: WholePlateTextStrategy,
) -> Result<String, LicensePlateError> {
    let new_text = validate_license_plate_text(new_text)?;
    let raw = trim_wrapping_quotes(raw_plate);
    let (content, suffix) = split_country_suffix(raw);
    let token_spans = tokenize_content(content)?;
    let text_segments = editable_text_segments(content, &token_spans);
    let replaced_content =
        replace_whole_text_segments(content, &text_segments, &new_text, strategy)?;
    Ok(join_country_suffix(&replaced_content, suffix))
}

pub fn edit_license_plate(
    raw_plate: &str,
    new_text: &str,
    text_color: Option<&str>,
    background_color: Option<&str>,
) -> Result<String, LicensePlateError> {
    let new_text = validate_license_plate_text(new_text)?;
    let raw = trim_wrapping_quotes(raw_plate);
    let (content, suffix) = split_country_suffix(raw);
    let token_spans = tokenize_content(content)?;
    let text_segments = editable_text_segments(content, &token_spans);
    if text_segments.is_empty() {
        return Err(LicensePlateError::UnsupportedFormat);
    }

    let first_text_start = text_segments[0].start;
    let mut working_content = content.to_string();
    apply_color_changes(
        &mut working_content,
        &token_spans,
        first_text_start,
        text_color,
        background_color,
    )?;

    let replaced_content = replace_whole_text_segments(
        &working_content,
        &text_segments,
        &new_text,
        WholePlateTextStrategy::ReplaceFirstSegmentAndClearRemaining,
    )?;
    Ok(join_country_suffix(&replaced_content, suffix))
}

fn trim_wrapping_quotes(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

fn split_country_suffix(raw: &str) -> (&str, Option<&str>) {
    raw.rsplit_once('|')
        .map(|(content, country)| (content, Some(country)))
        .unwrap_or((raw, None))
}

fn join_country_suffix(content: &str, suffix: Option<&str>) -> String {
    match suffix {
        Some(country) => format!("{content}|{country}"),
        None => content.to_string(),
    }
}

fn tokenize_content(content: &str) -> Result<Vec<TokenSpan>, LicensePlateError> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while cursor < content.len() {
        let remainder = &content[cursor..];
        if remainder.starts_with('<') {
            let Some(relative_end) = remainder.find('>') else {
                return Err(LicensePlateError::MalformedFormat);
            };
            let end = cursor + relative_end + 1;
            tokens.push(TokenSpan {
                kind: TokenKind::Tag,
                start: cursor,
                end,
            });
            cursor = end;
        } else {
            let end = remainder
                .find('<')
                .map(|relative| cursor + relative)
                .unwrap_or(content.len());
            tokens.push(TokenSpan {
                kind: TokenKind::Text,
                start: cursor,
                end,
            });
            cursor = end;
        }
    }

    Ok(tokens)
}

fn render_tokens(content: &str, tokens: &[TokenSpan]) -> Vec<PlateToken> {
    tokens
        .iter()
        .map(|token| match token.kind {
            TokenKind::Text => PlateToken::Text(content[token.start..token.end].to_string()),
            TokenKind::Tag => render_markup_token(&content[token.start..token.end]),
        })
        .collect()
}

fn render_markup_token(markup: &str) -> PlateToken {
    if is_tag_named(markup, "offset") {
        return PlateToken::Offset {
            hshift: find_attr_value(markup, "hshift").map(str::to_string),
            vshift: find_attr_value(markup, "vshift").map(str::to_string),
        };
    }

    if is_tag_named(markup, "img") {
        if let Some(src) = find_attr_value(markup, "src") {
            return PlateToken::Image {
                src: src.to_string(),
                color: find_attr_value(markup, "color").map(str::to_string),
            };
        }
    }

    PlateToken::RawMarkup(markup.to_string())
}

fn editable_text_segments(content: &str, tokens: &[TokenSpan]) -> Vec<TextSegment> {
    tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Text)
        .filter_map(|token| {
            let raw_text = &content[token.start..token.end];
            let leading_len = raw_text.len() - raw_text.trim_start().len();
            let trailing_len = raw_text.len() - raw_text.trim_end().len();
            let inner_start = token.start + leading_len;
            let inner_end = token.end.saturating_sub(trailing_len);
            if inner_start >= inner_end {
                return None;
            }
            let inner = &content[inner_start..inner_end];
            if inner.trim().is_empty() {
                return None;
            }
            Some(TextSegment {
                start: token.start,
                end: token.end,
                inner_start,
                inner_end,
            })
        })
        .collect()
}

fn replace_single_text_segment(content: &str, segment: &TextSegment, new_text: &str) -> String {
    let mut result = String::with_capacity(content.len() + new_text.len());
    result.push_str(&content[..segment.inner_start]);
    result.push_str(new_text);
    result.push_str(&content[segment.inner_end..]);
    result
}

fn replace_whole_text_segments(
    content: &str,
    segments: &[TextSegment],
    new_text: &str,
    strategy: WholePlateTextStrategy,
) -> Result<String, LicensePlateError> {
    if segments.is_empty() {
        return Err(LicensePlateError::UnsupportedFormat);
    }

    match strategy {
        WholePlateTextStrategy::ReplaceFirstSegmentAndClearRemaining => Ok(
            replace_first_text_segment_and_clear_remaining(content, segments, new_text),
        ),
    }
}

fn replace_first_text_segment_and_clear_remaining(
    content: &str,
    segments: &[TextSegment],
    new_text: &str,
) -> String {
    let mut result = String::with_capacity(content.len() + new_text.len());
    let mut cursor = 0;

    for (index, segment) in segments.iter().enumerate() {
        result.push_str(&content[cursor..segment.start]);
        result.push_str(&content[segment.start..segment.inner_start]);
        if index == 0 {
            result.push_str(new_text);
        }
        result.push_str(&content[segment.inner_end..segment.end]);
        cursor = segment.end;
    }

    result.push_str(&content[cursor..]);
    result
}

fn apply_color_changes(
    content: &mut String,
    tokens: &[TokenSpan],
    first_text_start: usize,
    text_color: Option<&str>,
    background_color: Option<&str>,
) -> Result<(), LicensePlateError> {
    if text_color.is_none() && background_color.is_none() {
        return Ok(());
    }

    let background_target = background_color
        .map(|_| find_background_color_target(content, tokens, first_text_start))
        .transpose()?
        .flatten();

    if let Some(color) = background_color {
        let target = background_target.ok_or(LicensePlateError::UnsupportedColor)?;
        replace_range(content, target, &parse_rgb_color(color)?.to_scs_argb());
    }

    if let Some(color) = text_color {
        let target = find_text_color_target(content, tokens, first_text_start, background_target)
            .ok_or(LicensePlateError::UnsupportedColor)?;
        replace_range(content, target, &parse_rgb_color(color)?.to_scs_argb());
    }

    Ok(())
}

fn find_text_color_target(
    content: &str,
    tokens: &[TokenSpan],
    first_text_start: usize,
    excluded: Option<ReplacementRange>,
) -> Option<ReplacementRange> {
    tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Tag && token.end <= first_text_start)
        .rev()
        .find_map(|token| {
            let tag = &content[token.start..token.end];
            if !is_tag_named(tag, "color") {
                return None;
            }
            let (start, end) = find_hex_attr_value_range(tag, "value")?;
            let target = ReplacementRange {
                start: token.start + start,
                end: token.start + end,
            };
            if Some(target) == excluded {
                None
            } else {
                Some(target)
            }
        })
}

fn find_background_color_target(
    content: &str,
    tokens: &[TokenSpan],
    first_text_start: usize,
) -> Result<Option<ReplacementRange>, LicensePlateError> {
    let img_index = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.kind == TokenKind::Tag && token.end <= first_text_start)
        .filter(|(_, token)| {
            let tag = &content[token.start..token.end];
            is_tag_named(tag, "img") && is_white_material_img(tag)
        })
        .map(|(index, _)| index)
        .last();

    let Some(img_index) = img_index else {
        return Ok(None);
    };

    let img = &tokens[img_index];
    let img_tag = &content[img.start..img.end];
    if let Some((start, end)) = find_hex_attr_value_range(img_tag, "color") {
        return Ok(Some(ReplacementRange {
            start: img.start + start,
            end: img.start + end,
        }));
    }

    Ok(tokens[..img_index].iter().rev().find_map(|token| {
        let tag = &content[token.start..token.end];
        if !is_tag_named(tag, "color") {
            return None;
        }
        let (start, end) = find_hex_attr_value_range(tag, "value")?;
        Some(ReplacementRange {
            start: token.start + start,
            end: token.start + end,
        })
    }))
}

fn replace_range(content: &mut String, range: ReplacementRange, replacement: &str) {
    content.replace_range(range.start..range.end, replacement);
}

fn is_tag_named(tag: &str, expected: &str) -> bool {
    let inner = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_start();
    if inner.starts_with('/') {
        return false;
    }
    inner
        .split_whitespace()
        .next()
        .map(|name| name.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

fn is_white_material_img(tag: &str) -> bool {
    let lower = tag.to_ascii_lowercase();
    is_tag_named(tag, "img") && lower.contains("/material/ui/white.mat")
}

fn find_attr_value<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let pattern = format!(r#"(?i)\b{}\s*=\s*("[^"]*"|[^\s>]+)"#, regex::escape(attr));
    let regex = Regex::new(&pattern).ok()?;
    let value = regex.captures(tag)?.get(1)?.as_str();
    Some(value.trim_matches('"'))
}

fn find_hex_attr_value_range(tag: &str, attr: &str) -> Option<(usize, usize)> {
    let pattern = format!(r#"(?i)\b{}\s*=\s*"?([0-9a-f]{{8}})"?"#, regex::escape(attr));
    let regex = Regex::new(&pattern).ok()?;
    regex
        .captures(tag)
        .and_then(|captures| captures.get(1).map(|match_| (match_.start(), match_.end())))
}

fn parse_rgb_color(value: &str) -> Result<RgbColor, LicensePlateError> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() == 6 && hex.chars().all(|character| character.is_ascii_hexdigit()) {
        let red =
            u8::from_str_radix(&hex[0..2], 16).map_err(|_| LicensePlateError::InvalidColor)?;
        let green =
            u8::from_str_radix(&hex[2..4], 16).map_err(|_| LicensePlateError::InvalidColor)?;
        let blue =
            u8::from_str_radix(&hex[4..6], 16).map_err(|_| LicensePlateError::InvalidColor)?;
        return Ok(RgbColor { red, green, blue });
    }

    let parts = trimmed
        .split(|character| matches!(character, ',' | '/' | ' '))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() == 3 {
        let values = parts
            .iter()
            .map(|part| {
                part.parse::<u8>()
                    .map_err(|_| LicensePlateError::InvalidColor)
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(RgbColor {
            red: values[0],
            green: values[1],
            blue: values[2],
        });
    }

    Err(LicensePlateError::InvalidColor)
}

impl RgbColor {
    fn to_scs_argb(self) -> String {
        format!("ff{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

fn fallback_display_text(raw_plate: &str) -> String {
    let raw = trim_wrapping_quotes(raw_plate);
    let content = split_country_suffix(raw).0;
    let mut output = String::new();
    let mut in_tag = false;
    for character in content.chars() {
        match character {
            '<' => {
                in_tag = true;
                output.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORWAY: &str = "<align right=148><font xscale=1.1 yscale=1.1>ZN 77957<offset vshift=-1> </font></align>|norway";
    const GERMANY_SPLIT: &str = "HH<offset hshift=-1 vshift=12><img src=/material/ui/lp/germany/b_hambur.mat color=FFFFFFFF><offset hshift=-16 vshift=-16><img src=/material/ui/lp/germany/h_ral6018_$SIDE$.mat color=FFFFFFFF><offset hshift=2 vshift=4>MY 825|germany";

    #[test]
    fn parses_norway_plate_text_and_country() {
        let parsed = parse_license_plate(NORWAY).unwrap();
        assert_eq!(parsed.country.as_deref(), Some("norway"));
        assert_eq!(parsed.visible_text, "ZN 77957");
        assert_eq!(serialize_without_changes(NORWAY).unwrap(), NORWAY);
    }

    #[test]
    fn replaces_norway_text_without_touching_formatting() {
        let updated = replace_license_plate_text(NORWAY, "ALEX").unwrap();
        assert_eq!(
            updated,
            "<align right=148><font xscale=1.1 yscale=1.1>ALEX<offset vshift=-1> </font></align>|norway"
        );
        assert!(updated.contains("right=148"));
        assert!(updated.contains("xscale=1.1"));
        assert!(updated.contains("yscale=1.1"));
        assert!(updated.contains("vshift=-1"));
        assert!(updated.ends_with("|norway"));
    }

    #[test]
    fn replaces_plain_plate_while_preserving_country_and_spacing() {
        let updated = replace_license_plate_text("DN 922CX   |italy", "ALEX").unwrap();
        assert_eq!(updated, "ALEX   |italy");
        assert_eq!(license_plate_display_text("GA65 LPV|uk"), "GA65 LPV");
    }

    #[test]
    fn handles_different_tag_order_and_multiple_text_fragments() {
        let raw = "DU<offset hshift=4 vshift=-5><img src=/material/ui/lp/germany/b.mat color=FFFFFFFF><offset hshift=4 vshift=5>GO 101|germany";
        let parsed = parse_license_plate(raw).unwrap();
        assert_eq!(parsed.visible_text, "DU GO 101");
        let updated = replace_license_plate_text(raw, "ALEX").unwrap();
        assert_eq!(
            updated,
            "ALEX<offset hshift=4 vshift=-5><img src=/material/ui/lp/germany/b.mat color=FFFFFFFF><offset hshift=4 vshift=5>|germany"
        );
    }

    #[test]
    fn parses_german_split_plate_as_ordered_render_stream() {
        let parsed = parse_license_plate(GERMANY_SPLIT).unwrap();
        assert_eq!(parsed.country.as_deref(), Some("germany"));
        assert_eq!(parsed.visible_text, "HH MY 825");

        let text_segments = parsed
            .tokens
            .iter()
            .filter_map(|token| match token {
                PlateToken::Text(value) if !value.trim().is_empty() => Some(value.trim()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(text_segments, vec!["HH", "MY 825"]);

        let image_assets = parsed
            .tokens
            .iter()
            .filter_map(|token| match token {
                PlateToken::Image { src, .. } => Some(src.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            image_assets,
            vec![
                "/material/ui/lp/germany/b_hambur.mat",
                "/material/ui/lp/germany/h_ral6018_$SIDE$.mat"
            ]
        );

        let offsets = parsed
            .tokens
            .iter()
            .filter_map(|token| match token {
                PlateToken::Offset { hshift, vshift } => {
                    Some((hshift.as_deref(), vshift.as_deref()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            offsets,
            vec![
                (Some("-1"), Some("12")),
                (Some("-16"), Some("-16")),
                (Some("2"), Some("4"))
            ]
        );

        let image_colors = parsed
            .tokens
            .iter()
            .filter_map(|token| match token {
                PlateToken::Image { color, .. } => Some(color.as_deref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(image_colors, vec![Some("FFFFFFFF"), Some("FFFFFFFF")]);
        assert_eq!(serialize_license_plate(&parsed), GERMANY_SPLIT);
        assert_eq!(
            serialize_without_changes(GERMANY_SPLIT).unwrap(),
            GERMANY_SPLIT
        );
    }

    #[test]
    fn replaces_german_segment_without_removing_prefix_text() {
        let updated = replace_plate_segment(GERMANY_SPLIT, 1, "ALEX").unwrap();
        assert_eq!(
            updated,
            "HH<offset hshift=-1 vshift=12><img src=/material/ui/lp/germany/b_hambur.mat color=FFFFFFFF><offset hshift=-16 vshift=-16><img src=/material/ui/lp/germany/h_ral6018_$SIDE$.mat color=FFFFFFFF><offset hshift=2 vshift=4>ALEX|germany"
        );
        assert!(updated.starts_with("HH<offset"));
        assert!(updated.contains("/material/ui/lp/germany/b_hambur.mat"));
        assert!(updated.contains("/material/ui/lp/germany/h_ral6018_$SIDE$.mat"));
        assert!(!updated.contains("MY 825"));
        assert_eq!(license_plate_display_text(&updated), "HH ALEX");
    }

    #[test]
    fn whole_replacement_for_german_split_plate_clears_all_original_text_segments() {
        let updated = replace_whole_plate_text(
            GERMANY_SPLIT,
            "ALEX",
            WholePlateTextStrategy::ReplaceFirstSegmentAndClearRemaining,
        )
        .unwrap();
        assert_eq!(
            updated,
            "ALEX<offset hshift=-1 vshift=12><img src=/material/ui/lp/germany/b_hambur.mat color=FFFFFFFF><offset hshift=-16 vshift=-16><img src=/material/ui/lp/germany/h_ral6018_$SIDE$.mat color=FFFFFFFF><offset hshift=2 vshift=4>|germany"
        );
        assert!(!updated.starts_with("HH"));
        assert!(!updated.contains("MY 825"));
        assert!(updated.contains("/material/ui/lp/germany/b_hambur.mat"));
        assert!(updated.contains("/material/ui/lp/germany/h_ral6018_$SIDE$.mat"));
        assert_eq!(license_plate_display_text(&updated), "ALEX");
    }

    #[test]
    fn accepts_special_plate_text_values() {
        for text in ["ALEX", "SIMNEXUS", "123 ABC"] {
            let updated = replace_license_plate_text("OLD|sweden", text).unwrap();
            assert_eq!(updated, format!("{text}|sweden"));
        }
    }

    #[test]
    fn rejects_markup_and_sii_string_breaking_input() {
        for text in ["\"", "\\", "<font>", "</align>"] {
            assert_eq!(
                replace_license_plate_text(NORWAY, text).unwrap_err(),
                LicensePlateError::InvalidText
            );
        }
    }

    #[test]
    fn updates_supported_text_and_background_colors() {
        let raw = "<margin left=-15><color value=ffffffff><img src=/material/ui/white.mat height=50 width=200><ret><offset hshift=-0.1 vshift=7.5><img src=/material/ui/white.mat height=35 width=155 color=ffff00ff><ret><offset hshift=0 vshift=14.5>  T-TOOLS|belgium";
        let updated = edit_license_plate(raw, "ALEX", Some("#000000"), Some("#ff00ff")).unwrap();
        assert_eq!(
            updated,
            "<margin left=-15><color value=ff000000><img src=/material/ui/white.mat height=50 width=200><ret><offset hshift=-0.1 vshift=7.5><img src=/material/ui/white.mat height=35 width=155 color=ffff00ff><ret><offset hshift=0 vshift=14.5>  ALEX|belgium"
        );
    }

    #[test]
    fn updates_background_color_tag_when_white_material_uses_active_color() {
        let raw = "<color value=ffde71ff><margin left=-15><img src=/material/ui/white.mat xscale=stretch yscale=stretch><ret><margin left=2><align hstyle=left vstyle=center><font xscale=1 yscale=1 ><color value=FFFFFFFF> GAY FURRY</align></margin>|belgium";
        let updated = edit_license_plate(raw, "ALEX", Some("#112233"), Some("#ffffff")).unwrap();
        assert_eq!(
            updated,
            "<color value=ffffffff><margin left=-15><img src=/material/ui/white.mat xscale=stretch yscale=stretch><ret><margin left=2><align hstyle=left vstyle=center><font xscale=1 yscale=1 ><color value=ff112233> ALEX</align></margin>|belgium"
        );
    }

    #[test]
    fn rejects_color_write_when_plate_has_no_supported_color_carrier() {
        assert_eq!(
            edit_license_plate("A 123|germany", "ALEX", Some("#ff0000"), None).unwrap_err(),
            LicensePlateError::UnsupportedColor
        );
    }

    #[test]
    fn rejects_malformed_unclosed_tag() {
        assert_eq!(
            parse_license_plate("<font A 123|germany").unwrap_err(),
            LicensePlateError::MalformedFormat
        );
    }
}
