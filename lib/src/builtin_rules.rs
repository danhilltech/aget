use crate::config::DomainRule;
use std::collections::HashMap;

/// Returns rules shipped with aget. User-supplied rules override these by domain key.
pub fn builtin_rules() -> HashMap<String, DomainRule> {
    let mut rules = HashMap::new();

    // github.com/{owner}/{repo} — fetch the default-branch README directly
    rules.insert(
        "github.com".to_string(),
        DomainRule {
            url_transform: Some(
                "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/README.md".to_string(),
            ),
            engine: Some("direct".to_string()),
            engines: None,
            headers: HashMap::new(),
            path_pattern: Some(r"^/[^/]+/[^/]+/?$".to_string()),
        },
    );

    // raw.githubusercontent.com — already plain text, skip the engine chain
    rules.insert(
        "raw.githubusercontent.com".to_string(),
        DomainRule {
            url_transform: None,
            engine: Some("direct".to_string()),
            engines: None,
            headers: HashMap::new(),
            path_pattern: None,
        },
    );

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_rule_present_with_expected_fields() {
        let rules = builtin_rules();
        let gh = rules.get("github.com").expect("github.com rule should exist");
        assert_eq!(gh.engine.as_deref(), Some("direct"));
        assert!(
            gh.url_transform.as_deref().unwrap().contains("raw.githubusercontent.com"),
            "transform should target raw.githubusercontent.com",
        );
        assert!(gh.path_pattern.is_some(), "github rule should be path-scoped");
    }

    #[test]
    fn test_raw_githubusercontent_present() {
        let rules = builtin_rules();
        let raw = rules
            .get("raw.githubusercontent.com")
            .expect("raw.githubusercontent.com rule should exist");
        assert_eq!(raw.engine.as_deref(), Some("direct"));
    }
}
