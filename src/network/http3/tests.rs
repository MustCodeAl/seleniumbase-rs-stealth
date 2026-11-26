use std::fmt::Write as _;
use std::net::IpAddr;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use crate::identity::fingerprint::{BrowserType, OsType};

use super::*;

/// A stand-in for `sbrs-h3d`: a shell script that writes each request line it reads to
/// `requests.txt` and answers with the lines given, in turn.
fn fake_helper(dir: &Path, answers: &[&str]) -> PathBuf {
    let mut script = String::from("#!/bin/sh\n");
    for answer in answers {
        script.push_str("IFS= read -r line || exit 0\n");
        writeln!(
            script,
            "printf '%s\\n' \"$line\" >> '{}'",
            dir.join("requests.txt").display()
        )
        .expect("a String takes any text");
        writeln!(script, "printf '%s\\n' '{answer}'").expect("a String takes any text");
    }
    let path = dir.join("fake-h3d");
    std::fs::write(&path, script).expect("the script is written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("it can run");
    path
}

#[tokio::test]
async fn a_missing_helper_says_how_to_install_it() {
    let error = Http3Client::start(&Http3Options::new().program("/nonexistent/sbrs-h3d"))
        .expect_err("missing");
    assert!(
        matches!(error, Http3Error::HelperMissing { .. }),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("cargo install --path h3d"),
        "{error}"
    );
}

#[tokio::test]
async fn a_request_is_one_line_and_its_answer_becomes_a_response() {
    let dir = tempfile::tempdir().expect("a folder");
    let program = fake_helper(
        dir.path(),
        &[
            r#"{"id":1,"status":201,"protocol":"h3","headers":[["X-Served-By","fake"]],"body":"aGk="}"#,
            r#"{"id":2,"error":{"kind":"send","message":"no route"}}"#,
        ],
    );
    let mut client = Http3Client::start(&Http3Options::new().program(&program)).expect("it starts");

    let request = Http3Request::new("POST", "https://localhost:4433/form")
        .header("content-type", "text/plain")
        .body("a=1")
        .timeout(Duration::from_millis(1500));
    let response = client.send(request).await.expect("a response");
    assert_eq!(
        (response.status(), response.protocol(), response.body()),
        (201, "h3", &b"hi"[..])
    );
    assert_eq!(response.header("x-served-by"), Some("fake"));

    let error = client
        .send(Http3Request::get("https://localhost:4433/"))
        .await
        .expect_err("a failure");
    assert!(
        matches!(&error, Http3Error::Failed { kind, .. } if kind == "send"),
        "{error:?}"
    );

    let written = std::fs::read_to_string(dir.path().join("requests.txt")).expect("the requests");
    let lines: Vec<serde_json::Value> = written
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON"))
        .collect();
    assert_eq!(
        lines[0],
        serde_json::json!({
            "id": 1, "method": "POST", "url": "https://localhost:4433/form",
            "headers": [["content-type", "text/plain"]], "body": "YT0x", "timeout_ms": 1500,
        })
    );
    assert_eq!(
        lines[1],
        serde_json::json!({ "id": 2, "method": "GET", "url": "https://localhost:4433/", "headers": [] })
    );
}

#[tokio::test]
async fn an_answer_for_another_request_is_refused() {
    let dir = tempfile::tempdir().expect("a folder");
    let program = fake_helper(
        dir.path(),
        &[r#"{"id":9,"status":200,"protocol":"h3","body":""}"#],
    );
    let mut client = Http3Client::start(&Http3Options::new().program(&program)).expect("it starts");
    let error = client
        .send(Http3Request::get("https://localhost/"))
        .await
        .expect_err("refused");
    assert!(matches!(error, Http3Error::Reply(_)), "{error:?}");
}

#[tokio::test]
async fn a_helper_that_ended_is_reported_as_ended() {
    let dir = tempfile::tempdir().expect("a folder");
    let program = fake_helper(dir.path(), &[]);
    let mut client = Http3Client::start(&Http3Options::new().program(&program)).expect("it starts");
    let error = client
        .send(Http3Request::get("https://localhost/"))
        .await
        .expect_err("ended");
    assert!(matches!(error, Http3Error::HelperEnded), "{error:?}");
}

#[test]
fn the_options_become_the_helpers_command_line() {
    let options = Http3Options::new()
        .profile("chrome-154-macos")
        .request_kind(Http3RequestKind::Subresource)
        .root_cert_der("/tmp/cert.der")
        .resolve("localhost", IpAddr::from([127, 0, 0, 1]))
        .proxy("socks5://127.0.0.1:1080");
    assert_eq!(
        options.arguments(),
        [
            "--profile",
            "chrome-154-macos",
            "--request-kind",
            "subresource",
            "--root-cert-der",
            "/tmp/cert.der",
            "--resolve",
            "localhost=127.0.0.1",
            "--proxy",
            "socks5://127.0.0.1:1080",
        ]
    );
}

#[test]
fn options_build_browser_selectors_and_proxy_shortcuts() {
    let chrome = Http3Options::new().for_browser(BrowserType::Chromium, 157, OsType::Windows);
    assert!(chrome
        .arguments()
        .contains(&"chrome-157-windows".to_owned()));

    let firefox = Http3Options::new().for_browser(BrowserType::Firefox, 157, OsType::Windows);
    assert!(firefox
        .arguments()
        .contains(&"firefox-157-windows".to_owned()));

    let safari = Http3Options::new().for_browser(BrowserType::MobileSafari, 26, OsType::Ios);
    assert!(safari.arguments().contains(&"safari-26-ios".to_owned()));

    let socks = Http3Options::new().socks5("127.0.0.1:1080");
    assert!(socks
        .arguments()
        .contains(&"socks5h://127.0.0.1:1080".to_owned()));

    let masque = Http3Options::new().connect_udp("masque.example:443");
    assert!(masque
        .arguments()
        .contains(&"connect-udp://masque.example:443".to_owned()));
}

#[tokio::test]
async fn pool_drops_exited_helper_before_returning_it() {
    let dir = tempfile::tempdir().expect("a folder");
    // First helper answers one request and exits immediately; second answers request 2.
    let program = fake_helper(
        dir.path(),
        &[r#"{"id":1,"status":200,"protocol":"h3","headers":[],"body":""}"#],
    );
    let pool = Http3Options::new().program(&program).pool(2);
    let mut client = pool.acquire().await.expect("starts");
    let resp = client
        .send(Http3Request::get("https://localhost/one"))
        .await
        .expect("first ok");
    assert_eq!(resp.status(), 200);
    // Wait briefly for the fake shell script to exit.
    tokio::time::sleep(Duration::from_millis(50)).await;
    pool.release(client).await;
    assert_eq!(pool.clients.lock().await.len(), 0);
}

#[test]
fn request_builder_helpers_set_headers_query_form_and_text_body() {
    let req = Http3Request::get("https://example.com/search")
        .headers([("x-one", "1"), ("x-two", "2")])
        .bearer_auth("tok-123")
        .referer("https://example.com/")
        .origin("https://example.com")
        .accept_language("en-US,en;q=0.9")
        .cookie("sid=abc")
        .query([("q", "hello world"), ("page", "2")]);

    assert_eq!(req.url, "https://example.com/search?q=hello+world&page=2");
    assert!(req.headers.contains(&("x-one".to_owned(), "1".to_owned())));
    assert!(req.headers.contains(&("x-two".to_owned(), "2".to_owned())));
    assert!(req
        .headers
        .contains(&("authorization".to_owned(), "Bearer tok-123".to_owned())));
    assert!(req
        .headers
        .contains(&("referer".to_owned(), "https://example.com/".to_owned())));
    assert!(req
        .headers
        .contains(&("origin".to_owned(), "https://example.com".to_owned())));
    assert!(req
        .headers
        .contains(&("accept-language".to_owned(), "en-US,en;q=0.9".to_owned())));
    assert!(req
        .headers
        .contains(&("cookie".to_owned(), "sid=abc".to_owned())));

    let basic = Http3Request::post("https://example.com/login")
        .basic_auth("alice", "secret")
        .form([("user", "alice"), ("role", "admin user")]);
    assert!(basic.headers.contains(&(
        "authorization".to_owned(),
        format!("Basic {}", STANDARD.encode("alice:secret"))
    )));
    assert_eq!(
        basic.body.as_deref(),
        Some(&b"user=alice&role=admin+user"[..])
    );
    assert!(basic.headers.contains(&(
        "content-type".to_owned(),
        "application/x-www-form-urlencoded".to_owned()
    )));

    let text = Http3Request::post("https://example.com/note").text_body("hello");
    assert_eq!(text.body.as_deref(), Some(&b"hello"[..]));
    assert!(text.headers.contains(&(
        "content-type".to_owned(),
        "text/plain; charset=utf-8".to_owned()
    )));
}

#[test]
fn response_helpers_inspect_headers_cookies_and_status_classes() {
    let resp = Http3Response {
        status: 201,
        protocol: "h3".to_owned(),
        headers: vec![
            ("Content-Type".to_owned(), "application/json".to_owned()),
            ("Set-Cookie".to_owned(), "a=1; Path=/".to_owned()),
            ("set-cookie".to_owned(), "b=2; HttpOnly".to_owned()),
        ],
        body: br#"{"ok":true}"#.to_vec(),
    };
    assert_eq!(resp.content_type(), Some("application/json"));
    assert_eq!(
        resp.headers_all("set-cookie"),
        vec!["a=1; Path=/", "b=2; HttpOnly"]
    );
    assert_eq!(resp.cookies(), vec!["a=1; Path=/", "b=2; HttpOnly"]);
    assert!(resp.is_success());
    assert!(!resp.is_redirect());
    assert!(!resp.is_client_error());
    assert!(!resp.is_server_error());

    let redir = Http3Response {
        status: 302,
        ..resp.clone()
    };
    assert!(redir.is_redirect());
    let client_err = Http3Response {
        status: 404,
        ..resp.clone()
    };
    assert!(client_err.is_client_error());
    let server_err = Http3Response {
        status: 503,
        ..resp
    };
    assert!(server_err.is_server_error());
}

#[tokio::test]
async fn pool_get_and_post_json_shorthands_work() {
    let dir = tempfile::tempdir().expect("a folder");
    let program = fake_helper(
        dir.path(),
        &[
            r#"{"id":1,"status":200,"protocol":"h3","headers":[],"body":"b2s="}"#,
            r#"{"id":2,"status":201,"protocol":"h3","headers":[],"body":"eyJvayI6dHJ1ZX0="}"#,
        ],
    );
    let pool = Http3Options::new().program(&program).pool(2);
    let r1 = pool.get("https://localhost/ping").await.expect("get ok");
    assert_eq!(r1.status(), 200);
    assert_eq!(r1.text().expect("utf8"), "ok");

    let r2 = pool
        .post_json("https://localhost/items", &serde_json::json!({"x": 1}))
        .await
        .expect("post_json ok");
    assert_eq!(r2.status(), 201);
    assert_eq!(
        r2.json::<serde_json::Value>().expect("json"),
        serde_json::json!({"ok": true})
    );
}
