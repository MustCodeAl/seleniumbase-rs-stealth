//! The identity headers of a profile: the user agent, the client hints and `Accept-Language`.

use seleniumbase_rs::evasion::fingerprint::ClientHints;
use seleniumbase_rs_codec::header_spec::ClientHeaders;

/// The locale whose `Accept-Language` a measured profile starts with.
pub(super) const DEFAULT_LOCALE_TAG: &str = "en-US";
/// The placeholder values for a user agent, an `Accept-Language` and a set of client hints.
pub(super) fn identity_for(
    user_agent: &str,
    accept_language: Option<&str>,
    hints: Option<&ClientHints>,
) -> ClientHeaders {
    let mut identity = ClientHeaders::new(user_agent);
    if let Some(language) = accept_language {
        identity = identity.with_accept_language(language);
    }
    if let Some(hints) = hints {
        identity = identity.with_client_hints_from(hints);
    }
    identity
}
