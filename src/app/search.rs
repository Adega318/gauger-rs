/// Case-insensitive fuzzy match: every query character must appear in the
/// name, in order, but not necessarily next to each other.
pub(crate) fn matches(query: &str, name: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let query = query.to_lowercase();
    let name = name.to_lowercase();
    let mut name_chars = name.chars();

    query
        .chars()
        .all(|query_char| name_chars.by_ref().any(|name_char| name_char == query_char))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything() {
        assert!(matches("", "postgres"));
        assert!(matches("", ""));
    }

    #[test]
    fn matches_exact_and_substring_names() {
        assert!(matches("postgres", "postgres"));
        assert!(matches("gres", "postgres"));
    }

    #[test]
    fn matches_fuzzy_subsequences() {
        assert!(matches("pgs", "postgres"));
        assert!(matches("myapi", "my-api"));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(matches("PGS", "Postgres"));
        assert!(matches("post", "POSTGRES"));
    }

    #[test]
    fn rejects_missing_or_out_of_order_characters() {
        assert!(!matches("postgresx", "postgres"));
        assert!(!matches("serg", "postgres"));
    }
}
