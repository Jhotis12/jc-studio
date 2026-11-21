//! Path claims: which repository prefixes a task expects to touch.
//!
//! Claims are directory prefixes, so overlap is a prefix relationship on segment
//! boundaries: `src/api` and `src/api/users` overlap; `src/a` and `src/ab` do not. An empty
//! claim says nothing, so it is treated as conflicting with everything that is running.

/// Whether two repository-relative prefixes could touch the same files.
pub fn paths_overlap(left: &str, right: &str) -> bool {
    let left = left.trim_matches('/');
    let right = right.trim_matches('/');

    if left.is_empty() || right.is_empty() {
        return true;
    }

    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|rest| rest.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether two claim sets could touch the same files.
pub fn claims_overlap(left: &[String], right: &[String]) -> bool {
    if left.is_empty() || right.is_empty() {
        return true;
    }
    left.iter()
        .any(|left| right.iter().any(|right| paths_overlap(left, right)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_prefixes_overlap_but_siblings_do_not() {
        assert!(paths_overlap("src/api", "src/api/users"));
        assert!(paths_overlap("src/api/users", "src/api"));
        assert!(!paths_overlap("src/a", "src/ab"));
        assert!(!paths_overlap("docs", "src"));
    }

    #[test]
    fn empty_claims_conflict_with_anything_running() {
        assert!(claims_overlap(&[], &["src".to_string()]));
        assert!(claims_overlap(&["src".to_string()], &[]));
        assert!(claims_overlap(
            &["src".to_string()],
            &["src/api".to_string()]
        ));
        assert!(!claims_overlap(&["src".to_string()], &["docs".to_string()]));
    }
}
