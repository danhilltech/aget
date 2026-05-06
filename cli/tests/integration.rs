use assert_cmd::Command;
use mockito::Server;

fn aget() -> Command {
    Command::cargo_bin("aget").unwrap()
}

#[tokio::test]
async fn test_fetches_native_markdown() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body("# Hello\n\nThis is markdown content that is long enough to pass quality and has **bold** text.\n\nMore content here to ensure we exceed the minimum character threshold.")
        .create_async()
        .await;

    let output = aget().arg(server.url()).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}

#[tokio::test]
async fn test_output_to_file() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body("# File Output Test\n\nLong enough content with **bold** and [links](http://example.com) here.")
        .create_async()
        .await;

    let out_file = tempfile::NamedTempFile::new().unwrap();
    let out_path = out_file.path().to_str().unwrap().to_string();

    let output = aget()
        .arg(server.url())
        .arg("-o")
        .arg(&out_path)
        .output()
        .unwrap();

    assert!(output.status.success());
    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(!content.is_empty());
}

#[test]
fn test_invalid_url_exits_nonzero() {
    let output = aget().arg("not-a-url").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_unknown_engine_exits_nonzero() {
    let output = aget()
        .arg("https://example.com")
        .arg("--engine")
        .arg("fake_engine")
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_help_exits_zero() {
    let output = aget().arg("--help").output().unwrap();
    assert!(output.status.success());
}

#[tokio::test]
async fn test_head_plain_text() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body(
            "# Test Title\n\nThis is the first paragraph of content with **bold** text and [a link](https://example.com). It is long enough to pass the quality check and contains markdown markers.",
        )
        .create_async()
        .await;

    let output = aget()
        .arg("--head")
        .arg("--no-cache")
        .arg(server.url())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("URL:"));
    assert!(stdout.contains("Engine:"));
    assert!(stdout.contains("Size:"));
    assert!(stdout.contains("Tokens:"));
    assert!(stdout.contains("Title:"));
    assert!(stdout.contains("Test Title"));
}

#[tokio::test]
async fn test_head_json() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body(
            "# Test Title\n\nThis is the first paragraph of content with **bold** text and [a link](https://example.com). It is long enough to pass the quality check and contains markdown markers.",
        )
        .create_async()
        .await;

    let output = aget()
        .arg("--head")
        .arg("--json")
        .arg("--no-cache")
        .arg(server.url())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(json.get("url").is_some());
    assert!(json.get("engine_used").is_some());
    assert!(json.get("size_bytes").is_some());
    assert!(json.get("size_kb").is_some());
    assert!(json.get("token_count").is_some());
    assert!(json.get("title").is_some());
    assert!(json.get("description").is_some());
}

#[test]
fn test_head_and_output_are_mutually_exclusive() {
    let output = aget()
        .arg("--head")
        .arg("-o")
        .arg("/tmp/aget-test-out.md")
        .arg("https://example.com")
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_json_without_head_exits_nonzero() {
    let output = aget()
        .arg("--json")
        .arg("https://example.com")
        .output()
        .unwrap();
    assert!(!output.status.success());
}
