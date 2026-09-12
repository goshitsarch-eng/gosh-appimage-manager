// Gosh AppImage Manager — drag-and-drop queue planning. Made by Gosh.
//
// Drops arrive per file from the toolkit; inspection confirms one file at a
// time. This pure helper decides whether the first dropped file starts
// immediately or everything waits behind current work, so the rule is tested
// without running a compositor.

/// Plan dropped files against current work.
///
/// - `queued`: files already waiting for confirmation.
/// - `busy`: a worker is running.
/// - `has_unconfirmed`: an inspection result is on screen awaiting a decision.
/// - `incoming`: newly dropped paths (already normalised, non-empty filtered
///   by the caller).
///
/// Returns `(to_start, new_queue)`: the file to inspect now, if any, and the
/// updated wait list. A busy worker or an unconfirmed result must never be
/// discarded by a drop.
pub fn plan_drop(
    queued: &[String],
    busy: bool,
    has_unconfirmed: bool,
    incoming: &[String],
) -> (Option<String>, Vec<String>) {
    let incoming: Vec<String> = incoming
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if incoming.is_empty() {
        return (None, queued.to_vec());
    }
    if busy || has_unconfirmed {
        let mut next = queued.to_vec();
        next.extend(incoming);
        return (None, next);
    }
    let mut next = queued.to_vec();
    let mut iter = incoming.into_iter();
    let first = iter.next();
    next.extend(iter);
    (first, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn idle_drop_starts_first_and_queues_rest() {
        let (start, queue) = plan_drop(&[], false, false, &s(&["a", "b", "c"]));
        assert_eq!(start.as_deref(), Some("a"));
        assert_eq!(queue, s(&["b", "c"]));
    }

    #[test]
    fn busy_drop_never_preempts() {
        let (start, queue) = plan_drop(&s(&["q"]), true, false, &s(&["a", "b"]));
        assert_eq!(start, None);
        assert_eq!(queue, s(&["q", "a", "b"]));
    }

    #[test]
    fn unconfirmed_result_is_not_discarded() {
        let (start, queue) = plan_drop(&[], false, true, &s(&["a"]));
        assert_eq!(start, None);
        assert_eq!(queue, s(&["a"]));
    }

    #[test]
    fn empty_drop_changes_nothing() {
        let (start, queue) = plan_drop(&s(&["q"]), false, false, &s(&["  ", ""]));
        assert_eq!(start, None);
        assert_eq!(queue, s(&["q"]));
    }
}
