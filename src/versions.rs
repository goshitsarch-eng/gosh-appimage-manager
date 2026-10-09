// Gosh AppImage Manager — release version comparison.
//
// An AppImage names its version in its desktop entry ("1.2.3") while the
// release that replaces it is named by a tag ("v1.2.3"). Comparing those as
// raw text made every app that was already current look outdated, and it
// offered an older release as an update to anyone ahead of the latest tag.
// This compares the way a person reads a version: a leading "v" is decoration,
// dotted numbers compare as numbers, and only a release that is actually newer
// is an update. Anything it cannot order -- a tag such as "continuous" -- is
// treated as different, not as newer or older.

use std::cmp::Ordering;

/// A version with the decoration a tag adds taken off: surrounding space, and a
/// leading "v" or "V" when a digit follows it ("v1.2" -> "1.2", "vim" is kept).
pub fn normalize(version: &str) -> &str {
    let trimmed = version.trim();
    match trimmed.strip_prefix(['v', 'V']) {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
        _ => trimmed,
    }
}

/// Whether two version labels name the same release.
pub fn same(a: &str, b: &str) -> bool {
    normalize(a).eq_ignore_ascii_case(normalize(b))
}

/// Dotted numbers split from whatever follows them: "1.2.3-rc1" is `[1, 2, 3]`
/// and "-rc1". `None` when the version does not begin with a number.
fn split_numeric(version: &str) -> Option<(Vec<u64>, &str)> {
    let mut numbers = Vec::new();
    let mut rest = version;
    loop {
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits == 0 {
            break;
        }
        numbers.push(rest[..digits].parse::<u64>().ok()?);
        rest = &rest[digits..];
        match rest.strip_prefix('.') {
            // A dot only continues the number when another digit follows it.
            Some(after) if after.starts_with(|c: char| c.is_ascii_digit()) => rest = after,
            _ => break,
        }
    }
    if numbers.is_empty() {
        None
    } else {
        Some((numbers, rest))
    }
}

/// How `offered` orders against `installed`, when both are versions that can be
/// ordered. `None` when either is not one (a date, a branch, "continuous").
pub fn order(installed: &str, offered: &str) -> Option<Ordering> {
    let (installed_numbers, installed_tail) = split_numeric(normalize(installed))?;
    let (offered_numbers, offered_tail) = split_numeric(normalize(offered))?;
    let width = installed_numbers.len().max(offered_numbers.len());
    for index in 0..width {
        let a = installed_numbers.get(index).copied().unwrap_or(0);
        let b = offered_numbers.get(index).copied().unwrap_or(0);
        match b.cmp(&a) {
            Ordering::Equal => {}
            unequal => return Some(unequal),
        }
    }
    // Same numbers. A pre-release tail ("-rc1", "-beta") comes before the plain
    // release; two different tails compare as text.
    let installed_tail = installed_tail.trim();
    let offered_tail = offered_tail.trim();
    Some(match (installed_tail.is_empty(), offered_tail.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => offered_tail
            .to_ascii_lowercase()
            .cmp(&installed_tail.to_ascii_lowercase()),
    })
}

/// Whether `offered` is a release to move to from `installed`.
///
/// * An empty offer says nothing, so it is never an update.
/// * An empty installed version (a file that never named one) takes any offer.
/// * The same label is not an update; the caller may still tell a rebuilt file
///   apart by its checksum.
/// * Versions that can be ordered are an update only when the offer is newer.
/// * Versions that cannot be ordered are an update when they differ.
pub fn is_update(installed: &str, offered: &str) -> bool {
    if normalize(offered).is_empty() {
        return false;
    }
    if normalize(installed).is_empty() {
        return true;
    }
    if same(installed, offered) {
        return false;
    }
    match order(installed, offered) {
        Some(ordering) => ordering == Ordering::Greater,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_v_is_decoration_only_before_a_digit() {
        assert_eq!(normalize("v1.2.3"), "1.2.3");
        assert_eq!(normalize(" V2 "), "2");
        assert_eq!(normalize("vim-9"), "vim-9");
        assert_eq!(normalize("continuous"), "continuous");
    }

    #[test]
    fn the_same_release_under_either_spelling_is_not_an_update() {
        assert!(!is_update("1.2.3", "v1.2.3"));
        assert!(!is_update("v1.2.3", "1.2.3"));
        assert!(!is_update("1.2.3", "1.2.3"));
        assert!(!is_update("Nightly", "nightly"));
    }

    #[test]
    fn numbers_compare_as_numbers() {
        assert!(is_update("1.9.0", "1.10.0"));
        assert!(!is_update("1.10.0", "1.9.0"));
        assert!(is_update("2", "10"));
        assert!(is_update("0.9.3", "0.14.2"));
    }

    #[test]
    fn a_missing_trailing_zero_is_the_same_release() {
        assert!(!is_update("2.4", "2.4.0"));
        assert!(!is_update("2.4.0", "2.4"));
        assert!(is_update("2.4", "2.4.1"));
    }

    #[test]
    fn an_older_release_is_not_an_update() {
        assert!(!is_update("2.0.0", "v1.9.0"));
        assert!(!is_update("1.0.1", "1.0.0"));
    }

    #[test]
    fn a_pre_release_comes_before_its_release() {
        assert!(is_update("1.0.0-rc1", "1.0.0"));
        assert!(!is_update("1.0.0", "1.0.0-rc2"));
        assert!(is_update("1.0.0-beta", "1.0.0-rc"));
        assert!(!is_update("1.0.0-rc", "1.0.0-beta"));
    }

    #[test]
    fn a_label_that_cannot_be_ordered_is_an_update_when_it_differs() {
        assert!(is_update("1.2.3", "continuous"));
        assert!(is_update("continuous", "1.2.3"));
        assert!(is_update("20240101", "nightly-20240102"));
        assert!(!is_update("continuous", "continuous"));
    }

    #[test]
    fn an_empty_side_is_handled() {
        assert!(!is_update("1.0", ""));
        assert!(!is_update("", ""));
        assert!(is_update("", "1.0"));
        assert!(is_update("  ", "v1.0"));
    }

    #[test]
    fn a_number_too_large_to_parse_is_left_unordered_not_a_panic() {
        let huge = "99999999999999999999999999.1";
        assert_eq!(order("1.0", huge), None);
        assert!(is_update("1.0", huge));
    }

    #[test]
    fn a_trailing_dot_does_not_swallow_the_tail() {
        assert_eq!(order("1.2.", "1.2.1"), Some(Ordering::Greater));
    }
}
