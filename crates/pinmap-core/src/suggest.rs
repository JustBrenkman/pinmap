//! "Did you mean" suggestions for names the user typed.

/// The name closest to `wanted`, ignoring case. Short names such as `PA5`
/// allow one edit, longer ones two, so unrelated names are not suggested.
pub fn suggest<'a>(wanted: &str, names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let wanted = wanted.to_ascii_uppercase();
    let limit = if wanted.chars().count() <= 4 { 1 } else { 2 };
    names
        .into_iter()
        .map(|name| (edit_distance(&wanted, &name.to_ascii_uppercase()), name))
        .filter(|(distance, _)| *distance <= limit)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, name)| name)
}

/// Levenshtein distance between two strings, by characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous[j] + usize::from(ca != *cb);
            current.push(substitute.min(previous[j + 1] + 1).min(current[j] + 1));
        }
        previous = current;
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_the_nearest_name() {
        let names = ["PA2", "PA5", "PB12"];
        assert_eq!(suggest("pa5", names), Some("PA5"));
        assert_eq!(suggest("PA55", names), Some("PA5"));
        assert_eq!(suggest("PC9", names), None);
        assert_eq!(suggest("OTG_FX", ["OTG_FS", "SPI1"]), Some("OTG_FS"));
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }
}
