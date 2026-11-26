//! Chromium command-line switch allowlist for external profile payloads.

/// Chrome switches a fingerprint payload may never set; the plugin policy refuses the same names.
///
/// A payload can come from outside the program, so a switch that runs a
/// program (`--renderer-cmd-prefix` puts any command in front of every
/// renderer), loads code, hands out remote control, or sends traffic
/// somewhere the caller did not choose is refused.
pub(crate) const FORBIDDEN_SWITCHES: &[&str] = &[
    "renderer-cmd-prefix",
    "utility-cmd-prefix",
    "zygote-cmd-prefix",
    "gpu-launcher",
    "ppapi-plugin-launcher",
    "plugin-launcher",
    "nacl-gdb",
    "nacl-gdb-script",
    "browser-subprocess-path",
    "load-extension",
    "load-component-extension",
    "user-data-dir",
    "remote-debugging-port",
    "remote-debugging-address",
    "remote-debugging-pipe",
    "proxy-server",
    "proxy-pac-url",
    "host-resolver-rules",
    "host-rules",
    "ignore-certificate-errors",
    "ignore-certificate-errors-spki-list",
];

/// The only Chrome switches a fingerprint payload may set: the ones that shape
/// the fingerprint or keep the browser quiet. A switch that is not here is
/// refused and logged, named or not, so a new Chrome switch that opens a door
/// (`--remote-allow-origins`, `--disable-web-security`, ...) needs no entry in
/// [`FORBIDDEN_SWITCHES`] to be kept out. Names are compared as written; Chrome
/// switches are lower case.
pub(crate) const PERMITTED_SWITCHES: &[&str] = &[
    // Language, agent, screen
    "lang",
    "accept-lang",
    "user-agent",
    "window-size",
    "window-position",
    "force-device-scale-factor",
    "force-color-profile",
    "high-dpi-support",
    "force-dark-mode",
    "hide-scrollbars",
    "start-maximized",
    // Features and blink
    "disable-features",
    "enable-features",
    "disable-blink-features",
    "enable-blink-features",
    // Network shape
    "disable-quic",
    "enable-quic",
    "force-webrtc-ip-handling-policy",
    "webrtc-ip-handling-policy",
    "enforce-webrtc-ip-permission-check",
    "disable-webrtc-hw-encoding",
    "disable-webrtc-hw-decoding",
    // Graphics and fonts
    "use-gl",
    "use-angle",
    "enable-webgl",
    "disable-webgl",
    "disable-gpu",
    "disable-gpu-compositing",
    "enable-unsafe-swiftshader",
    "ignore-gpu-blocklist",
    "disable-reading-from-canvas",
    "disable-3d-apis",
    "font-render-hinting",
    "disable-font-subpixel-positioning",
    "disable-lcd-text",
    "enable-font-antialiasing",
    // Audio and media
    "mute-audio",
    "autoplay-policy",
    // A quiet browser
    "disable-infobars",
    "no-first-run",
    "no-default-browser-check",
    "no-service-autorun",
    "no-pings",
    "disable-background-timer-throttling",
    "disable-backgrounding-occluded-windows",
    "disable-renderer-backgrounding",
    "disable-ipc-flooding-protection",
    "disable-hang-monitor",
    "disable-popup-blocking",
    "disable-prompt-on-repost",
    "disable-translate",
    "disable-sync",
    "disable-default-apps",
    "disable-component-update",
    "disable-notifications",
    "disable-session-crashed-bubble",
    "disable-search-engine-choice-screen",
    "disable-save-password-bubble",
    "disable-password-generation",
    "disable-single-click-autofill",
    "disable-client-side-phishing-detection",
    "safebrowsing-disable-download-protection",
    "password-store",
    "use-mock-keychain",
    "metrics-recording-only",
    "disable-breakpad",
    "disable-crash-reporter",
    "disable-domain-reliability",
    "disable-background-networking",
    "disable-component-extensions-with-background-pages",
    "disable-extensions",
    "disable-dev-shm-usage",
    "deny-permission-prompts",
    "disable-speech-api",
    "disable-device-discovery-notifications",
];

/// Whether a payload's `flag` (written without the leading dashes) and
/// `value` may be handed to Chrome.
///
/// The flag must be a plain switch name, the value must be one line, and the
/// switch must be one of `PERMITTED_SWITCHES` and none of `FORBIDDEN_SWITCHES`.
#[must_use]
pub fn permitted_switch(flag: &str, value: &str) -> bool {
    let plain = !flag.is_empty()
        && flag.starts_with(|c: char| c.is_ascii_alphanumeric())
        && flag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    plain
        && !value.contains(['\n', '\r', '\0'])
        && !FORBIDDEN_SWITCHES
            .iter()
            .any(|forbidden| forbidden.eq_ignore_ascii_case(flag))
        && PERMITTED_SWITCHES.contains(&flag)
}

/// The arguments for a payload's switches, in a fixed order, with the refused
/// ones left out.
#[must_use]
pub fn switches_to_args<'a>(switches: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    let mut kept: Vec<(&str, &str)> = switches
        .into_iter()
        .filter(|(flag, value)| {
            let ok = permitted_switch(flag, value);
            if !ok {
                tracing::warn!(flag, "refusing a Chrome switch from a fingerprint payload");
            }
            ok
        })
        .collect();
    kept.sort_unstable();
    kept.into_iter()
        .map(|(flag, value)| {
            if value.is_empty() {
                format!("--{flag}")
            } else {
                format!("--{flag}={value}")
            }
        })
        .collect()
}
