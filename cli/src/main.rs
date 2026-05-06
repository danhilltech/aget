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

    if let Some(shell) = cli.completions {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        let bin_name = cmd.get_name().to_string();
        clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        return Ok(());
    }

    let url_str = cli
        .url
        .as_deref()
        .expect("clap guarantees url is set when --completions is absent");
    let url = Url::parse(url_str).context("invalid URL")?;

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

    match (&cli.output, cli.chunk_size) {
        (Some(path), Some(max_chars)) => {
            let chunks = aget_lib::chunk::chunk_markdown(&result.content, max_chars);
            if chunks.len() == 1 {
                std::fs::write(path, &chunks[0])
                    .with_context(|| format!("failed to write to {}", path.display()))?;
            } else {
                let (stem, ext) = split_path(path);
                for (i, chunk) in chunks.iter().enumerate() {
                    let part_path = stem.with_file_name(format!(
                        "{}-{:03}{}",
                        stem.file_name().and_then(|s| s.to_str()).unwrap_or("output"),
                        i + 1,
                        ext.as_deref().unwrap_or(".md"),
                    ));
                    std::fs::write(&part_path, chunk)
                        .with_context(|| format!("failed to write to {}", part_path.display()))?;
                }
                eprintln!("[aget] wrote {} chunks", chunks.len());
            }
        }
        (Some(path), None) => {
            std::fs::write(path, &result.content)
                .with_context(|| format!("failed to write to {}", path.display()))?;
        }
        (None, _) => {
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

fn split_path(path: &std::path::Path) -> (std::path::PathBuf, Option<String>) {
    let stem = path
        .file_stem()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_else(|| std::ffi::OsString::from("output"));
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e));
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let stem_path = parent.join(stem);
    (stem_path, ext)
}
