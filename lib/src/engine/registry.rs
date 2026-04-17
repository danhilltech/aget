use crate::config::DomainRule;
use crate::engine::{
    accept_md::AcceptMdEngine, dot_md::DotMdEngine, html_extract::HtmlExtractEngine, Engine,
};

const DEFAULT_CHAIN: &[&str] = &["accept_md", "dot_md", "html_extract"];

pub fn engine_by_name(name: &str) -> Option<Box<dyn Engine>> {
    match name {
        "accept_md" => Some(Box::new(AcceptMdEngine)),
        "dot_md" => Some(Box::new(DotMdEngine)),
        "html_extract" => Some(Box::new(HtmlExtractEngine)),
        _ => None,
    }
}

pub fn build_chain(rule: Option<&DomainRule>) -> Vec<Box<dyn Engine>> {
    let names: &[&str] = if let Some(r) = rule {
        if let Some(ref engines) = r.engines {
            return engines.iter().filter_map(|n| engine_by_name(n)).collect();
        }
        DEFAULT_CHAIN
    } else {
        DEFAULT_CHAIN
    };

    names.iter().filter_map(|n| engine_by_name(n)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_chain_has_three_engines() {
        let chain = build_chain(None);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].name(), "accept_md");
        assert_eq!(chain[1].name(), "dot_md");
        assert_eq!(chain[2].name(), "html_extract");
    }

    #[test]
    fn test_domain_rule_overrides_chain() {
        let rule = DomainRule {
            engines: Some(vec!["dot_md".to_string(), "html_extract".to_string()]),
            ..Default::default()
        };
        let chain = build_chain(Some(&rule));
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].name(), "dot_md");
        assert_eq!(chain[1].name(), "html_extract");
    }

    #[test]
    fn test_unknown_engine_names_are_skipped() {
        let rule = DomainRule {
            engines: Some(vec!["accept_md".to_string(), "unknown_engine".to_string()]),
            ..Default::default()
        };
        let chain = build_chain(Some(&rule));
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].name(), "accept_md");
    }

    #[test]
    fn test_engine_by_name() {
        assert!(engine_by_name("accept_md").is_some());
        assert!(engine_by_name("dot_md").is_some());
        assert!(engine_by_name("html_extract").is_some());
        assert!(engine_by_name("nonexistent").is_none());
    }
}
