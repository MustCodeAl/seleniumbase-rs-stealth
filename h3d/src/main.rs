//! `sbrs-h3d`: HTTP/3 requests for the wire crate, sent by phantom-http with a browser profile.
//!
//! phantom builds its own `BoringSSL`, which cannot be linked into one binary with the wire
//! crate's (`docs/design/http3.md`), so the wire crate starts this program and talks to it over
//! stdin and stdout, one JSON object per line:
//!
//! ```text
//! sbrs-h3d [--profile NAME] [--request-kind navigation|subresource]
//!          [--proxy URL] [--root-cert-der PATH] [--resolve HOST=IP]...
//! ```
//!
//! A request line is `{"id": 1, "method": "GET", "url": "https://...", "headers": [["name",
//! "value"]], "body": "<base64>", "timeout_ms": 30000, "proxy": "socks5h://..."}` (only `id` and
//! `url` are required). Its answer is `{"id": 1, "status": 200, "protocol": "h3", "headers":
//! [["name", "value"]], "body": "<base64>"}`, or `{"id": 1, "error": {"kind": "...", "message":
//! "..."}}`. Every request is sent over HTTP/3 only, never falling back to HTTP/2 or HTTP/1.1.
//! Requests are answered in order, and the program ends when its input ends.
//!
//! `--root-cert-der` and `--resolve` exist for local test servers: an extra trusted certificate
//! and a fixed address for a host name.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use phantom::profile::catalog::{
    self, LayerSupport, ProfileDefinition, ProfileLayer, RequestTemplateKind,
};
use phantom::profile::{ClientProfile, RequestField};
use phantom::{
    Client, ConnectUdpProxy, HttpProtocol, RequestHeader, RequestTimeouts, ResponseInfo, Route,
    Socks5Proxy,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

/// The phantom profile used when none is named: the one the HTTP/3 spike compared with Chrome 156.
const DEFAULT_PROFILE: &str = "chrome-156-macos";

/// Default RFC 9298 path template appended to bare `connect-udp://host:port` endpoints.
const DEFAULT_MASQUE_PATH: &str = "/.well-known/masque/udp/{target_host}/{target_port}/";

/// Maximum distinct per-request proxy clients retained in one helper process.
const MAX_ROUTE_CLIENTS: usize = 16;

/// The largest response body read, in bytes; a longer one is an error.
const BODY_LIMIT: usize = 16 << 20;

/// The time limit of a request that names none.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// The longest a request may ask for.
const MAX_TIMEOUT: Duration = Duration::from_secs(300);

/// Request header template category selected on the command line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RequestKind {
    #[default]
    Navigation,
    Subresource,
}

impl RequestKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "navigation" => Ok(Self::Navigation),
            "subresource" => Ok(Self::Subresource),
            other => Err(format!(
                "--request-kind {other}: expected navigation or subresource"
            )),
        }
    }
}

/// The command line, parsed.
#[derive(Clone, Debug)]
struct Options {
    profile: String,
    request_kind: RequestKind,
    root_cert_der: Option<PathBuf>,
    resolve: Vec<(String, IpAddr)>,
    proxy: Option<String>,
}

/// One request line.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    #[serde(default)]
    method: Option<String>,
    url: String,
    #[serde(default)]
    headers: Vec<(String, String)>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    proxy: Option<String>,
}

/// One answer line.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Answer {
    Response {
        id: u64,
        status: u16,
        protocol: String,
        headers: Vec<(String, String)>,
        body: String,
    },
    Failure {
        id: Option<u64>,
        error: Failure,
    },
}

/// Why a request got no response.
#[derive(Debug, Serialize)]
struct Failure {
    /// `request` (the line or route could not be used), `send` (phantom failed), `timeout` or `body`.
    kind: &'static str,
    message: String,
}

impl Answer {
    fn failure(id: Option<u64>, kind: &'static str, message: impl Into<String>) -> Self {
        Answer::Failure {
            id,
            error: Failure {
                kind,
                message: message.into(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BrowserFamily {
    Chrome,
    Edge,
    Firefox,
    Brave,
    Opera,
    Safari,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlatformKind {
    Macos,
    Windows,
    Linux,
    Android,
    Ios,
}

impl PlatformKind {
    const fn is_mobile(self) -> bool {
        matches!(self, Self::Android | Self::Ios)
    }
}

struct CatalogEntry {
    selector: &'static str,
    browser: BrowserFamily,
    major: u32,
    platform: PlatformKind,
    intel_mac: bool,
}

const CATALOG_ENTRIES: &[CatalogEntry] = &[
    CatalogEntry {
        selector: "chrome-156-macos",
        browser: BrowserFamily::Chrome,
        major: 156,
        platform: PlatformKind::Macos,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "chrome-156-macos-intel",
        browser: BrowserFamily::Chrome,
        major: 156,
        platform: PlatformKind::Macos,
        intel_mac: true,
    },
    CatalogEntry {
        selector: "chrome-156-windows",
        browser: BrowserFamily::Chrome,
        major: 156,
        platform: PlatformKind::Windows,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "chrome-156-linux",
        browser: BrowserFamily::Chrome,
        major: 156,
        platform: PlatformKind::Linux,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "chrome-155-linux",
        browser: BrowserFamily::Chrome,
        major: 155,
        platform: PlatformKind::Linux,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "chrome-154-macos",
        browser: BrowserFamily::Chrome,
        major: 154,
        platform: PlatformKind::Macos,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "chrome-154-windows",
        browser: BrowserFamily::Chrome,
        major: 154,
        platform: PlatformKind::Windows,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "chrome-154-android",
        browser: BrowserFamily::Chrome,
        major: 154,
        platform: PlatformKind::Android,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "edge-154-windows",
        browser: BrowserFamily::Edge,
        major: 154,
        platform: PlatformKind::Windows,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "edge-153-windows",
        browser: BrowserFamily::Edge,
        major: 153,
        platform: PlatformKind::Windows,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "edge-153-android",
        browser: BrowserFamily::Edge,
        major: 153,
        platform: PlatformKind::Android,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "firefox-157-windows-r3",
        browser: BrowserFamily::Firefox,
        major: 157,
        platform: PlatformKind::Windows,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "firefox-156-android",
        browser: BrowserFamily::Firefox,
        major: 156,
        platform: PlatformKind::Android,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "brave-154-windows",
        browser: BrowserFamily::Brave,
        major: 154,
        platform: PlatformKind::Windows,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "brave-153-android",
        browser: BrowserFamily::Brave,
        major: 153,
        platform: PlatformKind::Android,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "opera-136-windows",
        browser: BrowserFamily::Opera,
        major: 136,
        platform: PlatformKind::Windows,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "opera-102-android",
        browser: BrowserFamily::Opera,
        major: 102,
        platform: PlatformKind::Android,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "safari-26-macos",
        browser: BrowserFamily::Safari,
        major: 26,
        platform: PlatformKind::Macos,
        intel_mac: false,
    },
    CatalogEntry {
        selector: "safari-26-ios",
        browser: BrowserFamily::Safari,
        major: 26,
        platform: PlatformKind::Ios,
        intel_mac: false,
    },
];

/// Resolves `name` to an exact `phantom-profile` catalog selector when present, or the closest
/// browser family and OS match in the compiled catalog.
fn resolve_profile(name: &str) -> &'static str {
    let trimmed = name.trim();
    if let Ok(definition) = catalog::profile(trimmed) {
        return definition.selector();
    }

    let mut browser: Option<BrowserFamily> = None;
    let mut platform: Option<PlatformKind> = None;
    let mut major: Option<u32> = None;
    let mut intel_mac = false;

    for token in trimmed
        .split(['-', '_', '/', ' '])
        .filter(|s| !s.is_empty())
    {
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "chrome" | "chromium" => browser = Some(BrowserFamily::Chrome),
            "edge" | "edg" | "msedge" => browser = Some(BrowserFamily::Edge),
            "firefox" | "gecko" | "fx" => browser = Some(BrowserFamily::Firefox),
            "brave" => browser = Some(BrowserFamily::Brave),
            "opera" | "opr" => browser = Some(BrowserFamily::Opera),
            "safari" | "mobilesafari" | "webkit" => browser = Some(BrowserFamily::Safari),
            "macos" | "mac" | "darwin" | "osx" => platform = Some(PlatformKind::Macos),
            "windows" | "win" | "win32" | "win64" => platform = Some(PlatformKind::Windows),
            "linux" | "x11" => platform = Some(PlatformKind::Linux),
            "android" => platform = Some(PlatformKind::Android),
            "ios" | "iphone" | "ipad" | "ipados" => platform = Some(PlatformKind::Ios),
            "intel" | "x86" | "x64" => intel_mac = true,
            _ => {
                if major.is_none()
                    && let Ok(parsed) = lower.parse::<u32>()
                    && (1..=999).contains(&parsed)
                {
                    major = Some(parsed);
                }
            }
        }
    }

    let wanted_browser = browser.unwrap_or(BrowserFamily::Chrome);
    CATALOG_ENTRIES
        .iter()
        .max_by_key(|entry| {
            let browser_tier: u8 = if entry.browser == wanted_browser {
                2
            } else {
                u8::from(entry.browser == BrowserFamily::Chrome)
            };
            let platform_tier: u8 = match platform {
                Some(p) if p == entry.platform => 3,
                None if entry.platform == PlatformKind::Macos => 2,
                Some(p)
                    if p.is_mobile() == entry.platform.is_mobile()
                        || (p == PlatformKind::Ios && entry.platform == PlatformKind::Macos) =>
                {
                    1
                }
                _ => 0,
            };
            let intel_tier = u8::from(entry.intel_mac == intel_mac);
            let version_closeness = major.map_or(u32::MAX, |m| u32::MAX - m.abs_diff(entry.major));
            (
                browser_tier,
                platform_tier,
                intel_tier,
                version_closeness,
                entry.major,
            )
        })
        .map_or(DEFAULT_PROFILE, |entry| entry.selector)
}

fn has_http3_layer(definition: &ProfileDefinition) -> bool {
    definition
        .evidence(ProfileLayer::Http3)
        .is_some_and(|evidence| evidence.support() == LayerSupport::Configured)
}

fn http3_transport_fallback(selector: &str) -> &'static str {
    match selector {
        "chrome-155-linux" => "chrome-156-linux",
        "firefox-156-android" => "firefox-157-windows-r3",
        "opera-102-android" => "chrome-154-android",
        _ => DEFAULT_PROFILE,
    }
}

fn default_user_agent(selector: &str) -> &'static str {
    match selector {
        "chrome-154-macos" => {
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36"
        }
        "edge-154-windows" => {
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36 Edg/154.0.0.0"
        }
        "edge-153-windows" => {
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36 Edg/153.0.0.0"
        }
        "brave-154-windows" => {
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36"
        }
        "opera-136-windows" => {
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36 OPR/136.0.0.0"
        }
        _ => {
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"
        }
    }
}

fn select_profile_and_defaults(
    requested_profile: &str,
    request_kind: RequestKind,
) -> Result<(ClientProfile, Vec<(String, String)>), String> {
    let resolved = resolve_profile(requested_profile);
    let mut definition = catalog::profile(resolved)
        .or_else(|_| catalog::profile(DEFAULT_PROFILE))
        .map_err(|error| format!("profile {requested_profile}: {error}"))?;
    let mut effective_selector = definition.selector();
    if !has_http3_layer(definition) {
        effective_selector = http3_transport_fallback(effective_selector);
        definition = catalog::profile(effective_selector)
            .map_err(|error| format!("profile {effective_selector}: {error}"))?;
    }

    let candidates: &[RequestTemplateKind] = match request_kind {
        RequestKind::Navigation => &[RequestTemplateKind::Navigation],
        RequestKind::Subresource => &[
            RequestTemplateKind::Fetch,
            RequestTemplateKind::FetchNoStore,
            RequestTemplateKind::Navigation,
        ],
    };
    let template = candidates
        .iter()
        .find_map(|&kind| {
            definition
                .request_template(kind)
                .ok()
                .filter(|tmpl| tmpl.http3_fields.is_some())
        })
        .ok_or_else(|| format!("profile {effective_selector} has no HTTP/3 request template"))?;

    let mut required_fallbacks = Vec::new();
    if let Some(fields) = &template.http3_fields {
        for field in fields {
            if let RequestField::Caller {
                name,
                required: true,
            } = field
            {
                if name.eq_ignore_ascii_case("user-agent") {
                    required_fallbacks.push((
                        name.to_string(),
                        default_user_agent(effective_selector).to_owned(),
                    ));
                } else if name.eq_ignore_ascii_case("accept-language") {
                    required_fallbacks.push((name.to_string(), "en-US,en;q=0.9".to_owned()));
                }
            }
        }
    }

    let profile = definition
        .instantiate(&[ProfileLayer::Http3])
        .map_err(|error| format!("profile {effective_selector}: {error}"))?
        .with_request_template(template);
    Ok((profile, required_fallbacks))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'%'
            && idx + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (
                char::from(bytes[idx + 1]).to_digit(16),
                char::from(bytes[idx + 2]).to_digit(16),
            )
            && let Ok(decoded) = u8::try_from((hi << 4) | lo)
        {
            out.push(decoded);
            idx += 3;
            continue;
        }
        out.push(bytes[idx]);
        idx += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn split_proxy_userinfo(rest: &str) -> (Option<(String, String)>, &str, &str) {
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    if let Some((userinfo, hostport)) = authority.rsplit_once('@') {
        let (user, pass) = userinfo.split_once(':').unwrap_or((userinfo, ""));
        (
            Some((percent_decode(user), percent_decode(pass))),
            hostport,
            tail,
        )
    } else {
        (None, authority, tail)
    }
}

/// Parses a proxy specification into a `phantom::Route` suitable for exact HTTP/3.
fn parse_route(spec: &str) -> Result<Route, String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("direct") {
        return Ok(Route::direct());
    }
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return Err(format!(
            "invalid proxy {trimmed:?}: expected socks5://, socks5h://, or connect-udp:// URL"
        ));
    };
    let lower_scheme = scheme.to_ascii_lowercase();
    match lower_scheme.as_str() {
        "http" => Err(
            "HTTP CONNECT proxy cannot carry QUIC/HTTP/3 (UDP required); use socks5://, socks5h://, or connect-udp://"
                .to_owned(),
        ),
        "socks4" | "socks4a" => Err(
            "SOCKS4 proxy cannot carry QUIC/HTTP/3 (UDP required); use socks5://, socks5h://, or connect-udp://"
                .to_owned(),
        ),
        "socks5" | "socks5h" => {
            let (creds, hostport, tail) = split_proxy_userinfo(rest);
            let clean = format!("{lower_scheme}://{hostport}{tail}");
            let mut proxy = Socks5Proxy::new(&clean).map_err(|error| format!("proxy: {error}"))?;
            if let Some((user, pass)) = creds {
                proxy = proxy
                    .with_username_password(user, pass)
                    .map_err(|error| format!("proxy credentials: {error}"))?;
            }
            Ok(Route::socks5(proxy))
        }
        "connect-udp" | "masque" => {
            let (creds, hostport, tail) = split_proxy_userinfo(rest);
            let path = if tail.is_empty() || tail == "/" {
                DEFAULT_MASQUE_PATH
            } else {
                tail
            };
            let template = format!("https://{hostport}{path}");
            let mut proxy =
                ConnectUdpProxy::new(&template).map_err(|error| format!("proxy: {error}"))?;
            if let Some((user, pass)) = creds {
                proxy = proxy
                    .with_basic_auth(user, pass)
                    .map_err(|error| format!("proxy credentials: {error}"))?;
            }
            Ok(Route::connect_udp(proxy))
        }
        "https" => {
            if !rest.contains("{target_host}") && !rest.contains("{target_port}") {
                return Err(
                    "HTTPS CONNECT proxy cannot carry QUIC/HTTP/3 without a MASQUE {target_host}/{target_port} template; use socks5://, socks5h://, or connect-udp://"
                        .to_owned(),
                );
            }
            let (creds, hostport, tail) = split_proxy_userinfo(rest);
            let template = format!("https://{hostport}{tail}");
            let mut proxy =
                ConnectUdpProxy::new(&template).map_err(|error| format!("proxy: {error}"))?;
            if let Some((user, pass)) = creds {
                proxy = proxy
                    .with_basic_auth(user, pass)
                    .map_err(|error| format!("proxy credentials: {error}"))?;
            }
            Ok(Route::connect_udp(proxy))
        }
        other => Err(format!(
            "unsupported proxy scheme {other:?}; expected socks5://, socks5h://, or connect-udp://"
        )),
    }
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        profile: DEFAULT_PROFILE.to_owned(),
        request_kind: RequestKind::Navigation,
        root_cert_der: None,
        resolve: Vec::new(),
        proxy: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => options.profile = args.next().ok_or("--profile needs a name")?,
            "--request-kind" => {
                let raw = args
                    .next()
                    .ok_or("--request-kind needs navigation or subresource")?;
                options.request_kind = RequestKind::parse(&raw)?;
            }
            "--proxy" => {
                options.proxy = Some(args.next().ok_or("--proxy needs an address or URL")?);
            }
            "--root-cert-der" => {
                options.root_cert_der = Some(PathBuf::from(
                    args.next().ok_or("--root-cert-der needs a path")?,
                ));
            }
            "--resolve" => {
                let spec = args.next().ok_or("--resolve needs HOST=IP")?;
                let (host, ip) = spec
                    .split_once('=')
                    .ok_or_else(|| format!("--resolve {spec}: write HOST=IP"))?;
                let ip: IpAddr = ip
                    .parse()
                    .map_err(|error| format!("--resolve {spec}: {error}"))?;
                options.resolve.push((host.to_owned(), ip));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

fn build_client(
    options: &Options,
    proxy_override: Option<&str>,
) -> Result<(Client, Vec<(String, String)>), String> {
    let (profile, required_fallbacks) =
        select_profile_and_defaults(&options.profile, options.request_kind)?;
    let mut builder =
        Client::builder(profile).request_timeouts(RequestTimeouts::new().total(MAX_TIMEOUT));
    if let Some(path) = &options.root_cert_der {
        let der = std::fs::read(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        builder = builder
            .add_root_certificate_der(der.clone())
            .add_proxy_root_certificate_der(der);
    }
    for (host, ip) in &options.resolve {
        builder = builder.resolve(host.as_str(), [*ip]);
    }
    let effective_proxy = proxy_override.or(options.proxy.as_deref());
    if let Some(spec) = effective_proxy {
        let route = parse_route(spec)?;
        builder = builder.route(route);
    }
    let built = builder.build().map_err(|error| error.to_string())?;
    Ok((built, required_fallbacks))
}

struct ClientPool {
    options: Options,
    default_client: Client,
    by_proxy: HashMap<String, Client>,
    required_fallbacks: Vec<(String, String)>,
}

impl ClientPool {
    fn new(options: Options) -> Result<Self, String> {
        let (default_client, required_fallbacks) = build_client(&options, None)?;
        Ok(Self {
            options,
            default_client,
            by_proxy: HashMap::new(),
            required_fallbacks,
        })
    }

    fn client_for(&mut self, request_proxy: Option<&str>) -> Result<&Client, String> {
        let Some(spec) = request_proxy else {
            return Ok(&self.default_client);
        };
        if self.options.proxy.as_deref() == Some(spec) {
            return Ok(&self.default_client);
        }
        if !self.by_proxy.contains_key(spec) {
            let (built, _) = build_client(&self.options, Some(spec))?;
            if self.by_proxy.len() >= MAX_ROUTE_CLIENTS {
                self.by_proxy.clear();
            }
            self.by_proxy.insert(spec.to_owned(), built);
        }
        self.by_proxy
            .get(spec)
            .ok_or_else(|| "proxy client cache lookup failed".to_owned())
    }
}

/// The error and its causes, one after the other.
fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// The protocol a response came over, as ALPN names it.
fn protocol_name(info: Option<&ResponseInfo>) -> String {
    match info.map(|info| format!("{:?}", info.protocol())) {
        Some(name) if name == "Http3" => "h3".to_owned(),
        Some(name) if name == "Http2" => "h2".to_owned(),
        Some(name) if name == "Http1" => "http/1.1".to_owned(),
        Some(name) => name.to_lowercase(),
        None => "unknown".to_owned(),
    }
}

async fn answer(pool: &mut ClientPool, request: Request) -> Answer {
    let id = Some(request.id);
    let method = request.method.as_deref().unwrap_or("GET");
    let Ok(method) = http::Method::from_bytes(method.as_bytes()) else {
        return Answer::failure(id, "request", format!("{method} is not an HTTP method"));
    };
    let body = match request
        .body
        .as_deref()
        .map(|body| STANDARD.decode(body))
        .transpose()
    {
        Ok(body) => body,
        Err(error) => {
            return Answer::failure(id, "request", format!("the body is not base64: {error}"))
        }
    };
    let fallbacks = pool.required_fallbacks.clone();
    let client = match pool.client_for(request.proxy.as_deref()) {
        Ok(client) => client,
        Err(error) => return Answer::failure(id, "request", error),
    };
    let mut builder = match client.request_with_protocol(HttpProtocol::Http3, method, &request.url)
    {
        Ok(builder) => builder,
        Err(error) => return Answer::failure(id, "request", chain(&error)),
    };
    for (req_name, fallback_val) in &fallbacks {
        if !request
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(req_name))
        {
            builder = builder.header(RequestHeader::new(req_name.as_str(), fallback_val.as_str()));
        }
    }
    for (name, value) in request.headers {
        builder = builder.header(RequestHeader::new(name, value));
    }
    if let Some(body) = body {
        builder = builder.body(body);
    }
    let limit = request
        .timeout_ms
        .map_or(DEFAULT_TIMEOUT, Duration::from_millis)
        .min(MAX_TIMEOUT);
    let response = match tokio::time::timeout(limit, builder.send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return Answer::failure(id, "send", chain(&error)),
        Err(_elapsed) => {
            return Answer::failure(
                id,
                "timeout",
                format!("no response within {} ms", limit.as_millis()),
            );
        }
    };
    let status = response.status().as_u16();
    let protocol = protocol_name(response.extensions().get::<ResponseInfo>());
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    match response.into_body().collect_with_limit(BODY_LIMIT).await {
        Ok(body) => Answer::Response {
            id: request.id,
            status,
            protocol,
            headers,
            body: STANDARD.encode(&body),
        },
        Err(error) => Answer::failure(id, "body", chain(&error)),
    }
}

async fn serve(mut pool: ClientPool) -> std::io::Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Request>(&line) {
            Ok(request) => answer(&mut pool, request).await,
            Err(error) => Answer::failure(None, "request", format!("not a request: {error}")),
        };
        let mut text = serde_json::to_string(&answer).map_err(std::io::Error::other)?;
        text.push('\n');
        stdout.write_all(text.as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("sbrs-h3d: {error}");
            return ExitCode::from(2);
        }
    };
    let pool = match ClientPool::new(options) {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("sbrs-h3d: {error}");
            return ExitCode::from(2);
        }
    };
    match serve(pool).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sbrs-h3d: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Options, String> {
        parse_args(list.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn the_command_line_takes_profile_request_kind_proxy_cert_and_addresses() {
        let options = args(&[
            "--profile",
            "chrome-154-macos",
            "--request-kind",
            "subresource",
            "--proxy",
            "socks5h://127.0.0.1:1080",
            "--root-cert-der",
            "/tmp/cert.der",
            "--resolve",
            "localhost=127.0.0.1",
        ])
        .expect("valid");
        assert_eq!(options.profile, "chrome-154-macos");
        assert_eq!(options.request_kind, RequestKind::Subresource);
        assert_eq!(options.proxy.as_deref(), Some("socks5h://127.0.0.1:1080"));
        assert_eq!(options.root_cert_der, Some(PathBuf::from("/tmp/cert.der")));
        assert_eq!(
            options.resolve,
            [("localhost".to_owned(), IpAddr::from([127, 0, 0, 1]))]
        );
        let defaults = args(&[]).expect("valid");
        assert_eq!(defaults.profile, DEFAULT_PROFILE);
        assert_eq!(defaults.request_kind, RequestKind::Navigation);
    }

    #[test]
    fn a_wrong_command_line_says_what_is_wrong() {
        assert!(args(&["--resolve", "localhost"]).is_err_and(|error| error.contains("HOST=IP")));
        assert!(args(&["--resolve", "localhost=nowhere"]).is_err());
        assert!(args(&["--profile"]).is_err());
        assert!(args(&["--request-kind"]).is_err());
        assert!(args(&["--request-kind", "websocket"]).is_err());
        assert!(args(&["--nosuch"]).is_err_and(|error| error.contains("--nosuch")));
    }

    #[test]
    fn resolve_profile_matches_exact_catalog_and_closest_browser_os() {
        assert_eq!(resolve_profile("chrome-156-macos"), "chrome-156-macos");
        assert_eq!(
            resolve_profile("chrome-156-macos-intel"),
            "chrome-156-macos-intel"
        );
        assert_eq!(resolve_profile("chrome-155-linux"), "chrome-155-linux");
        assert_eq!(resolve_profile("chrome-157-windows"), "chrome-156-windows");
        assert_eq!(resolve_profile("chrome-153-windows"), "chrome-154-windows");
        assert_eq!(resolve_profile("chrome-157-android"), "chrome-154-android");
        assert_eq!(resolve_profile("chrome-157-linux"), "chrome-156-linux");
        assert_eq!(resolve_profile("edge-157-windows"), "edge-154-windows");
        assert_eq!(resolve_profile("edge-152-windows"), "edge-153-windows");
        assert_eq!(resolve_profile("edge-156-android"), "edge-153-android");
        assert_eq!(
            resolve_profile("firefox-157-windows"),
            "firefox-157-windows-r3"
        );
        assert_eq!(
            resolve_profile("firefox-158-linux"),
            "firefox-157-windows-r3"
        );
        assert_eq!(
            resolve_profile("firefox-158-android"),
            "firefox-156-android"
        );
        assert_eq!(resolve_profile("brave-156-windows"), "brave-154-windows");
        assert_eq!(resolve_profile("brave-156-android"), "brave-153-android");
        assert_eq!(resolve_profile("opera-140-windows"), "opera-136-windows");
        assert_eq!(resolve_profile("opera-110-android"), "opera-102-android");
        assert_eq!(resolve_profile("safari-26-macos"), "safari-26-macos");
        assert_eq!(resolve_profile("safari-26-ios"), "safari-26-ios");
        assert_eq!(resolve_profile("mobilesafari-18-ios"), "safari-26-ios");
    }

    #[test]
    fn every_catalog_profile_builds_an_http3_client_for_navigation_and_subresource() {
        for entry in CATALOG_ENTRIES {
            for kind in [RequestKind::Navigation, RequestKind::Subresource] {
                let options = Options {
                    profile: entry.selector.to_owned(),
                    request_kind: kind,
                    root_cert_der: None,
                    resolve: Vec::new(),
                    proxy: None,
                };
                assert!(
                    build_client(&options, None).is_ok(),
                    "failed for {} {kind:?}",
                    entry.selector
                );
            }
        }
    }

    #[test]
    fn parse_route_wires_socks5_and_connect_udp_and_rejects_tcp_connect_proxies() {
        assert_eq!(parse_route("direct").expect("direct"), Route::direct());
        assert!(matches!(
            parse_route("socks5://127.0.0.1:1080").expect("socks5"),
            Route::Socks5(_)
        ));
        assert!(matches!(
            parse_route("socks5h://alice:s3cret@127.0.0.1:1080").expect("socks5h auth"),
            Route::Socks5(_)
        ));
        assert!(matches!(
            parse_route("connect-udp://proxy.example:443").expect("bare connect-udp"),
            Route::ConnectUdp(_)
        ));
        assert!(matches!(
            parse_route(
                "https://alice:s3cret@proxy.example:8443/.well-known/masque/udp/{target_host}/{target_port}/"
            )
            .expect("masque https"),
            Route::ConnectUdp(_)
        ));
        let tcp_err = parse_route("http://127.0.0.1:8080").expect_err("tcp connect rejected");
        assert!(tcp_err.contains("UDP"), "{tcp_err}");
        let https_err = parse_route("https://proxy.example:8443").expect_err("https connect");
        assert!(https_err.contains("MASQUE"), "{https_err}");
    }

    #[test]
    fn a_request_line_needs_an_id_and_a_url_and_nothing_unknown() {
        let request: Request =
            serde_json::from_str(r#"{"id":7,"url":"https://localhost/"}"#).expect("minimal");
        assert_eq!((request.id, request.method, request.body), (7, None, None));
        assert!(serde_json::from_str::<Request>(r#"{"url":"https://localhost/"}"#).is_err());
        assert!(
            serde_json::from_str::<Request>(r#"{"id":1,"url":"x","unknown_field":"y"}"#).is_err()
        );
        let with_proxy: Request =
            serde_json::from_str(r#"{"id":1,"url":"x","proxy":"socks5://127.0.0.1:1080"}"#)
                .expect("proxy");
        assert_eq!(with_proxy.proxy.as_deref(), Some("socks5://127.0.0.1:1080"));
    }

    #[test]
    fn answers_are_one_json_object_each() {
        let failure = Answer::failure(Some(3), "timeout", "no response within 10 ms");
        assert_eq!(
            serde_json::to_value(&failure).expect("json"),
            serde_json::json!({ "id": 3, "error": { "kind": "timeout", "message": "no response within 10 ms" } })
        );
        let response = Answer::Response {
            id: 4,
            status: 204,
            protocol: "h3".to_owned(),
            headers: vec![("server".to_owned(), "local".to_owned())],
            body: String::new(),
        };
        assert_eq!(
            serde_json::to_value(&response).expect("json"),
            serde_json::json!({ "id": 4, "status": 204, "protocol": "h3", "headers": [["server", "local"]], "body": "" })
        );
    }
}
