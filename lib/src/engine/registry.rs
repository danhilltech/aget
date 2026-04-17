use crate::config::DomainRule;
use crate::engine::{
    accept_md::AcceptMdEngine, dot_md::DotMdEngine, html_extract::HtmlExtractEngine, Engine,
};

pub fn engine_by_name(name: &str) -> Option<Box<dyn Engine>> {
    match name {
        "accept_md" => Some(Box::new(AcceptMdEngine)),
        "dot_md" => Some(Box::new(DotMdEngine)),
        "html_extract" => Some(Box::new(HtmlExtractEngine)),
        _ => None,
    }
}

pub fn build_chain(_rule: Option<&DomainRule>) -> Vec<Box<dyn Engine>> {
    vec![
        Box::new(AcceptMdEngine),
        Box::new(DotMdEngine),
        Box::new(HtmlExtractEngine),
    ]
}
