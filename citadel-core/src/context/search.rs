/// Extract likely symbol names from a natural language query.
///
/// Supports patterns:
/// - CamelCase: UserService, signInWithGoogle
/// - snake_case: user_service, sign_in
/// - SCREAMING_SNAKE: MAX_RETRIES
/// - dot.notation: app.isPackaged (extracts both sides)
/// - ALL_CAPS acronyms: HTTP, REST, LRU
/// - Single words that look like identifiers
pub fn extract_symbols_from_query(query: &str) -> Vec<String> {
    let mut symbols = std::collections::HashSet::new();

    // CamelCase: upper followed by lower, or lower followed by upper+lower
    // e.g., UserService, signInWithGoogle
    let camel_re = regex_lite::Regex::new(r"\b([A-Z][a-z]+(?:[A-Z][a-z]*)*|[a-z]+(?:[A-Z][a-z]*)+)\b").unwrap();
    for cap in camel_re.captures_iter(query) {
        if let Some(m) = cap.get(1)
            && m.as_str().len() >= 2 {
                symbols.insert(m.as_str().to_string());
            }
    }

    // snake_case
    let snake_re = regex_lite::Regex::new(r"\b([a-z][a-z0-9]*(?:_[a-z0-9]+)+)\b").unwrap();
    for cap in snake_re.captures_iter(query) {
        if let Some(m) = cap.get(1)
            && m.as_str().len() >= 3 {
                symbols.insert(m.as_str().to_string());
            }
    }

    // SCREAMING_SNAKE_CASE
    let screaming_re = regex_lite::Regex::new(r"\b([A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+)\b").unwrap();
    for cap in screaming_re.captures_iter(query) {
        if let Some(m) = cap.get(1) {
            symbols.insert(m.as_str().to_string());
        }
    }

    // ALL_CAPS acronyms (2+ chars)
    let acronym_re = regex_lite::Regex::new(r"\b([A-Z]{2,})\b").unwrap();
    for cap in acronym_re.captures_iter(query) {
        if let Some(m) = cap.get(1) {
            symbols.insert(m.as_str().to_string());
        }
    }

    // dot.notation: split "app.isPackaged" → ["app", "isPackaged"]
    let dot_re = regex_lite::Regex::new(r"\b([a-zA-Z][a-zA-Z0-9]*(?:\.[a-zA-Z][a-zA-Z0-9]*)+)\b").unwrap();
    for cap in dot_re.captures_iter(query) {
        if let Some(m) = cap.get(1) {
            for part in m.as_str().split('.') {
                if !part.is_empty() && part.chars().next().is_some_and(|c| c.is_alphabetic()) {
                    symbols.insert(part.to_string());
                }
            }
        }
    }

    symbols.into_iter().collect()
}

/// Split a CamelCase name into its component words.
/// e.g., "UserService" → ["User", "Service"]
pub fn split_camel_case(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_uppercase() && chars[i - 1].is_lowercase() {
            words.push(current.clone());
            current.clear();
        }
        current.push(c);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Score how well a node name matches multiple query terms.
pub fn compound_term_score(node_name: &str, query_terms: &[&str]) -> f64 {
    let name_lower = node_name.to_lowercase();
    let mut matched = 0;
    for term in query_terms {
        if name_lower.contains(&term.to_lowercase()) {
            matched += 1;
        }
    }
    if query_terms.is_empty() { 0.0 } else { matched as f64 / query_terms.len() as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_camelcase() {
        let symbols = extract_symbols_from_query("How does UserService handle signInWithGoogle?");
        assert!(symbols.contains(&"UserService".to_string()));
        assert!(symbols.contains(&"signInWithGoogle".to_string()));
    }

    #[test]
    fn test_extract_snake_case() {
        let symbols = extract_symbols_from_query("The user_service handles sign_in requests");
        assert!(symbols.contains(&"user_service".to_string()));
        assert!(symbols.contains(&"sign_in".to_string()));
    }

    #[test]
    fn test_extract_dot_notation() {
        let symbols = extract_symbols_from_query("Check app.isPackaged before launch");
        assert!(symbols.contains(&"app".to_string()));
        assert!(symbols.contains(&"isPackaged".to_string()));
    }

    #[test]
    fn test_split_camel_case() {
        let words = split_camel_case("UserService");
        assert_eq!(words, vec!["User", "Service"]);
    }

    #[test]
    fn test_compound_term_score() {
        let score = compound_term_score("UserAuthenticationService", &["user", "auth"]);
        assert!(score > 0.5);
    }
}
