#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyDisposition {
    Merge,
    AutoMerge,
    ManualReview,
    KeepSeparate,
}

pub fn levenshtein_distance(left: &str, right: &str) -> usize {
    if left == right {
        return 0;
    }
    if left.is_empty() {
        return right.chars().count();
    }
    if right.is_empty() {
        return left.chars().count();
    }

    let right_chars: Vec<char> = right.chars().collect();
    let left_chars: Vec<char> = left.chars().collect();
    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (i, left_char) in left_chars.iter().enumerate() {
        current[0] = i + 1;
        for (j, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            current[j + 1] = (current[j] + 1)
                .min(previous[j + 1] + 1)
                .min(previous[j] + substitution_cost);
        }
        previous.clone_from(&current);
    }

    previous[right_chars.len()]
}

pub fn levenshtein_similarity(left: &str, right: &str) -> f64 {
    let left = left.trim().to_ascii_lowercase();
    let right = right.trim().to_ascii_lowercase();
    if left.is_empty() && right.is_empty() {
        return 100.0;
    }
    let max_len = left.chars().count().max(right.chars().count());
    if max_len == 0 {
        return 100.0;
    }
    let distance = levenshtein_distance(&left, &right);
    ((max_len.saturating_sub(distance)) as f64 / max_len as f64) * 100.0
}

pub fn fuzzy_disposition(similarity: f64) -> FuzzyDisposition {
    if similarity >= 100.0 {
        FuzzyDisposition::Merge
    } else if similarity >= 95.0 {
        FuzzyDisposition::AutoMerge
    } else if similarity >= 85.0 {
        FuzzyDisposition::ManualReview
    } else {
        FuzzyDisposition::KeepSeparate
    }
}

#[cfg(test)]
mod tests {
    use super::{FuzzyDisposition, fuzzy_disposition, levenshtein_similarity};

    #[test]
    fn fuzzy_merge_thresholds() {
        assert_eq!(fuzzy_disposition(100.0), FuzzyDisposition::Merge);
        assert_eq!(fuzzy_disposition(97.0), FuzzyDisposition::AutoMerge);
        assert_eq!(fuzzy_disposition(90.0), FuzzyDisposition::ManualReview);
        assert_eq!(fuzzy_disposition(70.0), FuzzyDisposition::KeepSeparate);
        assert!(levenshtein_similarity("Berlin", "Berln") >= 83.0);
    }
}
