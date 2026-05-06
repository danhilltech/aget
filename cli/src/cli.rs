use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "aget")]
#[command(about = "Fetch a URL and output its content as Markdown")]
#[command(version)]
pub struct Cli {
    /// URL to fetch and convert to Markdown
    pub url: String,

    /// Write output to FILE instead of stdout
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Config file path
    #[arg(short = 'C', long = "config", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Print engine attempts and quality results to stderr
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Force a specific engine (overrides domain rules): accept_md, dot_md, html_extract
    #[arg(long = "engine", value_name = "NAME")]
    pub engine: Option<String>,

    /// Disable HTTP response caching
    #[arg(long = "no-cache")]
    pub no_cache: bool,

    /// Print a content summary (size, tokens, title) instead of outputting Markdown
    #[arg(long = "head", conflicts_with = "output")]
    pub head: bool,

    /// Output --head result as JSON
    #[arg(long = "json", requires = "head")]
    pub json: bool,

    /// Split output into multiple files of this max char count (requires --output)
    #[arg(
        long = "chunk-size",
        value_name = "N",
        requires = "output",
        conflicts_with = "head"
    )]
    pub chunk_size: Option<usize>,
}
