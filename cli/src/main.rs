mod cli;

use aget_lib::{
    config::{Config, DomainRule},
    engine::registry::engine_by_name,
    head::head,
    pipeline::Pipeline,
};
use anyhow::{Context, Result};
use clap::Parser;
use cli::Cli;
use std::io::Write;
use url::Url;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("aget: {:#}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.json && !cli.head {
        eprintln!("aget: warning: --json has no effect without --head");
    }

    let url = Url::parse(&cli.url).context("invalid URL")?;

    let config = match &cli.config {
        Some(path) => Config::load(path).context("failed to load config")?,
        None => Config::load_default().context("failed to load default config")?,
    };

    let domain = url.host_str().unwrap_or("").to_string();
    let mut rule: Option<DomainRule> = config.domains.get(&domain).cloned();

    if let Some(ref engine_name) = cli.engine {
        if engine_by_name(engine_name).is_none() {
            anyhow::bail!(
                "unknown engine '{}'. Valid: accept_md, dot_md, html_extract",
                engine_name
            );
        }
        rule = Some(DomainRule {
            engines: Some(vec![engine_name.clone()]),
            headers: rule.as_ref().map(|r| r.headers.clone()).unwrap_or_default(),
            ..Default::default()
        });
    }

    let pipeline = Pipeline::new(cli.no_cache).context("failed to create pipeline")?;

    if cli.head {
        let result = head(&url, &pipeline, rule.as_ref())
            .await
            .context("head failed")?;
        let output = if cli.json {
            result.to_json()
        } else {
            result.to_plain_text()
        };
        println!("{}", output);
        return Ok(());
    }

    let result = pipeline
        .run(&url, rule.as_ref(), cli.verbose)
        .await
        .context("fetch failed")?;

    match &cli.output {
        Some(path) => {
            std::fs::write(path, &result.content)
                .with_context(|| format!("failed to write to {}", path.display()))?;
        }
        None => {
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            out.write_all(result.content.as_bytes())
                .context("failed to write to stdout")?;
            if !result.content.ends_with('\n') {
                out.write_all(b"\n").ok();
            }
        }
    }

    Ok(())
}
