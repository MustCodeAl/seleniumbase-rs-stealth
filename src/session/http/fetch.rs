//! The hop loop: sends a request, stores its cookies, follows redirects by hand, and reads the
//! body within the limits.
//!
//! One request runs under one deadline that covers every hop and the body. Each hop waits for its
//! site's pace, sends the headers of the profile for the protocol the origin speaks with the
//! `Cookie` header the jar gives for that hop's URL, stores the `Set-Cookie` headers of the
//! answer, and either follows the redirect (see [`redirect`](super::redirect)) or reads the body.
//! The text of an error from the HTTP client is dropped: it can name the URL or the proxy.

use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use tokio::time::{Instant, timeout_at};
use url::Url;
use wreq::Method;
use wreq::header::SET_COOKIE;

use super::error::HttpError;
use super::options::MAX_TIMEOUT;
use super::redirect::{Hop, site_name, target_of};
use super::reply::{HttpResponse, ResponseHeaders};
use super::request::{Prepared, merge_headers};
use super::{HttpSession, MAX_REMEMBERED_ORIGINS};
use crate::network::convert;
use crate::network::proxy::redact::RedactedUrl;
use crate::network::spec::header_spec::RequestShape;
use crate::network::wire::{WireFormatError, WireProtocol};
use crate::session::challenge::mitigation::FetchKind;
use crate::session::ledger::events::SessionEvent;

/// The largest buffer reserved for a body before its bytes arrive, whatever the server claims.
const MAX_PRESIZE_BYTES: usize = 1024 * 1024;
/// How much of the body of a redirect is read so that its connection can be used again.
const MAX_REDIRECT_DRAIN_BYTES: usize = 64 * 1024;
/// The header the transport writes first on HTTP/1.1.
const HOST_HEADER: &str = "Host";
/// What a failure to meet the deadline says.
const TIMED_OUT: &str = "timed out";

/// What a hop got back, before its body is read.
struct Answer {
    status: u16,
    headers: ResponseHeaders,
    protocol: WireProtocol,
    body_source: HopBody,
}

enum HopBody {
    Wreq(Box<wreq::Response>),
    #[cfg_attr(
        not(feature = "http3"),
        allow(
            dead_code,
            reason = "constructed only when the http3 feature is enabled"
        )
    )]
    Direct(Bytes),
}

/// Runs a prepared request to its reply.
///
/// # Errors
///
/// See [`HttpRequest::send`](super::HttpRequest::send).
pub(super) async fn run(
    session: &HttpSession,
    prepared: Prepared,
) -> Result<HttpResponse, HttpError> {
    let limits = *session.limits();
    let started = Instant::now();
    let budget = prepared.timeout.unwrap_or(limits.timeout).min(MAX_TIMEOUT);
    let deadline = started + budget;
    let shape = prepared.shape;
    let mut hop = Hop::first(prepared);
    let mut chain: Vec<RedactedUrl> = Vec::new();
    loop {
        let answer = exchange(session, &hop, shape, deadline).await?;
        let target = target_of(
            answer.status,
            &answer.headers,
            &hop.url,
            limits.follow_redirects,
        )?;
        let Some(next) = target else {
            let body = read_body(&hop, answer.body_source, limits.max_body_bytes, deadline).await?;
            let reply = HttpResponse {
                status: answer.status,
                url: hop.url,
                headers: answer.headers,
                body,
                protocol: answer.protocol,
                redirects: chain,
                elapsed: started.elapsed(),
            };
            announce(session, &reply);
            return Ok(reply);
        };
        if chain.len() >= limits.max_redirects {
            return Err(HttpError::TooManyRedirects(limits.max_redirects));
        }
        drain(answer.body_source, deadline).await;
        chain.push(RedactedUrl::new(next.as_str()));
        hop = hop.follow(answer.status, next);
    }
}

/// Emits the event of a finished request.
fn announce(session: &HttpSession, reply: &HttpResponse) {
    let elapsed_ms = u64::try_from(reply.elapsed.as_millis()).unwrap_or(u64::MAX);
    session.emit(&SessionEvent::Request {
        url: reply.redacted_url(),
        status: reply.status,
        fetch_kind: FetchKind::Http,
        elapsed_ms,
    });
}

/// Sends one hop and stores the cookies of its answer.
async fn exchange(
    session: &HttpSession,
    hop: &Hop,
    shape: RequestShape,
    deadline: Instant,
) -> Result<Answer, HttpError> {
    #[cfg(feature = "http3")]
    if hop.protocol == Some(WireProtocol::Http3) {
        return exchange_http3(session, hop, shape, deadline).await;
    }
    #[cfg(not(feature = "http3"))]
    if hop.protocol == Some(WireProtocol::Http3) {
        return Err(HttpError::Client {
            kind: "HTTP/3 requested but the `http3` feature is not enabled",
        });
    }

    #[cfg(feature = "http3")]
    if hop.protocol.is_none()
        && hop.url.scheme() == "https"
        && session.inner().http3_alt_svc
        && has_alt_svc_h3(session, &hop.url)
    {
        match exchange_http3(session, hop, shape, deadline).await {
            Ok(answer) => return Ok(answer),
            Err(error) => {
                tracing::debug!(
                    ?error,
                    "opportunistic Alt-Svc HTTP/3 failed; falling back to HTTP/2"
                );
                clear_alt_svc_h3(session, &hop.url);
            }
        }
    }

    let inner = session.inner();
    if let Some(pacer) = &inner.pacer {
        pacer.acquire(&site_name(&hop.url)).await;
    }
    let context = super::cookies::context_for(shape, &hop.method, hop.crossed_site);
    let cookie = inner.jar.header_for(&hop.url, context, session.now());
    let wire = wire_for(session, &hop.url);
    let request = build_request(session, hop, shape, cookie.as_deref(), wire)?;
    let response = timeout_at(deadline, request.send())
        .await
        .map_err(|_elapsed| failure(&hop.url, TIMED_OUT))?
        .map_err(|error| failure(&hop.url, classify(&error)))?;
    let protocol = if response.version() == wreq::Version::HTTP_2 {
        WireProtocol::Http2
    } else if response.version() == wreq::Version::HTTP_3 {
        WireProtocol::Http3
    } else {
        WireProtocol::Http1
    };
    remember(session, &hop.url, protocol);
    let headers = headers_of(response.headers());
    store_cookies(session, &hop.url, &headers);
    if inner.http3_alt_svc {
        record_alt_svc(session, &hop.url, &headers);
    }
    Ok(Answer {
        status: response.status().as_u16(),
        headers,
        protocol,
        body_source: HopBody::Wreq(Box::new(response)),
    })
}

#[cfg(feature = "http3")]
async fn exchange_http3(
    session: &HttpSession,
    hop: &Hop,
    shape: RequestShape,
    deadline: Instant,
) -> Result<Answer, HttpError> {
    use crate::network::http3::{Http3Options, Http3Pool, Http3Request, Http3RequestKind};
    let inner = session.inner();
    if let Some(pacer) = &inner.pacer {
        pacer.acquire(&site_name(&hop.url)).await;
    }
    let context = super::cookies::context_for(shape, &hop.method, hop.crossed_site);
    let cookie = inner.jar.header_for(&hop.url, context, session.now());
    let mut req = Http3Request::new(hop.method.as_str(), hop.url.as_str());
    if let Some(cookie_val) = cookie {
        req = req.header("cookie", cookie_val);
    }
    for (name, val) in &hop.headers {
        req = req.header(name.to_ascii_lowercase(), val);
    }
    if let Some(body) = &hop.body {
        req = req.body(body.to_vec());
    }
    let now = Instant::now();
    let timeout = if deadline > now {
        deadline - now
    } else {
        std::time::Duration::from_millis(1)
    };
    req = req.timeout(timeout);

    let profile = &inner.profile;
    let request_kind = match shape {
        RequestShape::Document => Http3RequestKind::Navigation,
        RequestShape::Fetch => Http3RequestKind::Subresource,
    };
    let mut options = Http3Options::new()
        .for_version(profile.major(), profile.os())
        .request_kind(request_kind);
    if let Some(cert_path) = &inner.root_cert_der {
        options = options.root_cert_der(cert_path);
    }
    for (host, addr) in &inner.resolve {
        options = options.resolve(host, addr.ip());
    }
    if let Some(egress) = &inner.egress {
        options = options.proxy(egress.key().to_string());
    }

    let pool = {
        let mut pools = crate::sync::locked(&inner.http3_pools);
        if let Some(existing) = pools.get(&options) {
            existing.clone()
        } else {
            let created = Http3Pool::new(options.clone());
            if pools.len() >= MAX_REMEMBERED_ORIGINS {
                pools.clear();
            }
            pools.insert(options, created.clone());
            created
        }
    };
    let resp = pool
        .send(req)
        .await
        .map_err(|_err| failure(&hop.url, "http3 send failed"))?;
    let headers = ResponseHeaders::new(resp.headers().to_vec());
    store_cookies(session, &hop.url, &headers);
    if inner.http3_alt_svc {
        record_alt_svc(session, &hop.url, &headers);
    }
    remember(session, &hop.url, WireProtocol::Http3);
    Ok(Answer {
        status: resp.status(),
        headers,
        protocol: WireProtocol::Http3,
        body_source: HopBody::Direct(Bytes::from(resp.body().to_vec())),
    })
}

/// Default `Alt-Svc` lifetime when `ma=` is omitted (RFC 7838 section 3.1: 24 hours).
const DEFAULT_ALT_SVC_MAX_AGE_SECS: u64 = 86_400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AltSvcH3 {
    None,
    Clear,
    Advertised { max_age_secs: u64 },
}

fn parse_alt_svc_h3(header_value: &str) -> AltSvcH3 {
    let trimmed = header_value.trim();
    if trimmed.eq_ignore_ascii_case("clear") {
        return AltSvcH3::Clear;
    }
    for entry in trimmed.split(',') {
        let mut parts = entry.split(';');
        let Some(proto_alt) = parts.next().map(str::trim) else {
            continue;
        };
        let Some((proto, _alt)) = proto_alt.split_once('=') else {
            continue;
        };
        if !proto.trim().eq_ignore_ascii_case("h3") {
            continue;
        }
        let mut max_age = DEFAULT_ALT_SVC_MAX_AGE_SECS;
        for param in parts {
            if let Some((k, v)) = param.trim().split_once('=')
                && k.trim().eq_ignore_ascii_case("ma")
            {
                let v_clean = v.trim().trim_matches('"');
                if let Ok(parsed) = v_clean.parse::<u64>() {
                    max_age = parsed;
                }
            }
        }
        return AltSvcH3::Advertised {
            max_age_secs: max_age,
        };
    }
    AltSvcH3::None
}

fn record_alt_svc(session: &HttpSession, url: &Url, headers: &ResponseHeaders) {
    if url.scheme() != "https" {
        return;
    }
    let now = session.now();
    for value in headers.get_all("alt-svc") {
        match parse_alt_svc_h3(value) {
            AltSvcH3::Clear => clear_alt_svc_h3(session, url),
            AltSvcH3::Advertised { max_age_secs } => {
                if max_age_secs == 0 {
                    clear_alt_svc_h3(session, url);
                } else {
                    let mut table = crate::sync::locked(&session.inner().alt_svc_h3);
                    if table.len() >= MAX_REMEMBERED_ORIGINS {
                        table.clear();
                    }
                    table.insert(origin_key(url), now.saturating_add(max_age_secs));
                }
            }
            AltSvcH3::None => {}
        }
    }
}

#[cfg(feature = "http3")]
fn has_alt_svc_h3(session: &HttpSession, url: &Url) -> bool {
    let now = session.now();
    let mut table = crate::sync::locked(&session.inner().alt_svc_h3);
    let key = origin_key(url);
    match table.get(&key).copied() {
        Some(expires_at) if expires_at > now => true,
        Some(_) => {
            table.remove(&key);
            false
        }
        None => false,
    }
}

fn clear_alt_svc_h3(session: &HttpSession, url: &Url) {
    let mut table = crate::sync::locked(&session.inner().alt_svc_h3);
    table.remove(&origin_key(url));
}

/// The headers of a response with lower-case names; a value that is not UTF-8 is read with the
/// broken bytes replaced.
fn headers_of(map: &wreq::header::HeaderMap) -> ResponseHeaders {
    ResponseHeaders::new(
        map.iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect(),
    )
}

/// Stores the `Set-Cookie` headers of an answer. A header that is not UTF-8 is ignored, as a
/// browser ignores it.
fn store_cookies(session: &HttpSession, url: &Url, headers: &ResponseHeaders) {
    let lines: Vec<&str> = headers.get_all(SET_COOKIE.as_str()).collect();
    if lines.is_empty() {
        return;
    }
    let absorbed = session.inner().jar.absorb(url, &lines, session.now());
    tracing::debug!(
        stored = absorbed.stored,
        deleted = absorbed.deleted,
        ignored = absorbed.ignored,
        "set-cookie headers read"
    );
}

/// The protocol to write headers for: the one the origin settled on, HTTP/2 for an origin not met
/// yet (nearly every site speaks it), and HTTP/1.1 for a plain `http` URL.
fn wire_for(session: &HttpSession, url: &Url) -> WireProtocol {
    if url.scheme() == "http" {
        return WireProtocol::Http1;
    }
    let table = crate::sync::locked(&session.inner().protocols);
    table
        .get(&origin_key(url))
        .copied()
        .unwrap_or(WireProtocol::Http2)
}

/// Remembers the protocol an origin answered on.
fn remember(session: &HttpSession, url: &Url, wire: WireProtocol) {
    let mut table = crate::sync::locked(&session.inner().protocols);
    if table.len() >= MAX_REMEMBERED_ORIGINS {
        table.clear();
    }
    table.insert(origin_key(url), wire);
}

fn origin_key(url: &Url) -> String {
    url.origin().ascii_serialization()
}

/// The wreq request for one hop: the profile's headers for `wire` with the cookie and the
/// caller's headers in, in the profile's order.
fn build_request(
    session: &HttpSession,
    hop: &Hop,
    shape: RequestShape,
    cookie: Option<&str>,
    wire: WireProtocol,
) -> Result<wreq::RequestBuilder, HttpError> {
    let inner = session.inner();
    let mut list = inner.profile.headers_for(shape, cookie, wire)?;
    merge_headers(&mut list, &hop.headers);
    let leading: &[&'static str] = match wire {
        WireProtocol::Http1 => &[HOST_HEADER],
        WireProtocol::Http2 | WireProtocol::Http3 => &[],
    };
    let (headers, order) = convert::wreq_headers(&list, leading).map_err(WireFormatError::from)?;
    let mut request = inner
        .client
        .request(hop.method.clone(), hop.url.as_str())
        .default_headers(false)
        .headers(headers)
        .orig_headers(order);
    if let Some(body) = &hop.body {
        request = request.body(body.clone());
    }
    Ok(request)
}

/// Reads the short body of a redirect, so that its connection goes back to the pool. A long or
/// slow body is dropped, which closes the connection.
async fn drain(body_source: HopBody, deadline: Instant) {
    if let HopBody::Wreq(response) = body_source {
        let mut stream = std::pin::pin!(response.bytes_stream());
        let mut seen = 0_usize;
        while let Ok(Some(Ok(chunk))) = timeout_at(deadline, stream.next()).await {
            seen = seen.saturating_add(chunk.len());
            if seen > MAX_REDIRECT_DRAIN_BYTES {
                break;
            }
        }
    }
}

/// Reads the body of the last hop within `limit` bytes, counted after decompression.
async fn read_body(
    hop: &Hop,
    body_source: HopBody,
    limit: usize,
    deadline: Instant,
) -> Result<Bytes, HttpError> {
    match body_source {
        HopBody::Direct(bytes) => {
            if hop.method != Method::HEAD && bytes.len() > limit {
                return Err(HttpError::BodyTooLarge(limit));
            }
            Ok(bytes)
        }
        HopBody::Wreq(response) => {
            let promised = response.content_length();
            if hop.method != Method::HEAD && promised.is_some_and(|bytes| bytes > limit as u64) {
                return Err(HttpError::BodyTooLarge(limit));
            }
            let presize = promised
                .and_then(|bytes| usize::try_from(bytes).ok())
                .unwrap_or(0)
                .min(limit)
                .min(MAX_PRESIZE_BYTES);
            let mut body = BytesMut::with_capacity(presize);
            let mut stream = std::pin::pin!(response.bytes_stream());
            while let Some(chunk) = timeout_at(deadline, stream.next())
                .await
                .map_err(|_elapsed| failure(&hop.url, TIMED_OUT))?
            {
                let chunk = chunk.map_err(|error| failure(&hop.url, classify(&error)))?;
                if body.len().saturating_add(chunk.len()) > limit {
                    return Err(HttpError::BodyTooLarge(limit));
                }
                body.extend_from_slice(&chunk);
            }
            Ok(body.freeze())
        }
    }
}

/// A failed request, in the words of [`HttpError::Request`].
fn failure(url: &Url, kind: &'static str) -> HttpError {
    HttpError::Request {
        url: RedactedUrl::new(url.as_str()),
        kind,
    }
}

/// What kind of failure an HTTP client error is, in a word or two. The error's own text can name
/// the URL and the proxy, so it is never kept.
fn classify(error: &wreq::Error) -> &'static str {
    if error.is_timeout() {
        TIMED_OUT
    } else if error.is_proxy_connect() {
        "the proxy refused or failed the connection"
    } else if error.is_dns() {
        "the host name could not be resolved"
    } else if error.is_tls() {
        "the TLS handshake or the certificate failed"
    } else if error.is_connect() {
        "the connection could not be made"
    } else if error.is_connection_reset() {
        "the connection was reset"
    } else if error.is_decode() {
        "the body could not be decoded"
    } else if error.is_body() {
        "the body could not be read"
    } else if error.is_builder() {
        "the request could not be built"
    } else {
        "the request failed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alt_svc_parser_extracts_h3_max_age_and_clear() {
        assert_eq!(parse_alt_svc_h3("clear"), AltSvcH3::Clear);
        assert_eq!(
            parse_alt_svc_h3(r#"h3=":443""#),
            AltSvcH3::Advertised {
                max_age_secs: DEFAULT_ALT_SVC_MAX_AGE_SECS
            }
        );
        assert_eq!(
            parse_alt_svc_h3(r#"h3-29=":443"; ma=3600, h3=":443"; ma=7200"#),
            AltSvcH3::Advertised { max_age_secs: 7200 }
        );
        assert_eq!(parse_alt_svc_h3(r#"h2=":443"; ma=3600"#), AltSvcH3::None);
    }
}
