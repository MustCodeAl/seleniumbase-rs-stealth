//! The mitigation policy that turns a detection and the attempt context into one next `Mitigation`.
//!
//! The policy is a pure function: it reads no clock (the caller passes `now`) and keeps no
//! state (the caller passes how many challenges in a row the site has shown). The decision
//! table, in the order it is applied:
//!
//! | Detection | Decision |
//! |---|---|
//! | [`ChallengeKind::Clear`] | [`Mitigation::Proceed`] |
//! | any other, and `attempt >= max_challenge_attempts` | [`Mitigation::GiveUp`] |
//! | [`ChallengeKind::RateLimited`] | [`Mitigation::Cooldown`] for `Retry-After`, capped |
//! | [`ChallengeKind::Blocked`] | [`Mitigation::RotateProxy`], or [`Mitigation::Cooldown`] when it cannot |
//! | script or interactive challenge in [`FetchKind::Http`] | [`Mitigation::Escalate`] to the browser |
//! | script or interactive challenge in [`FetchKind::Browser`] | [`Mitigation::Solve`], then rotate |
//!
//! The rotation ladder rises with `consecutive_challenges`: when the `solve` options say a proxy
//! rotation is due (`rotate_proxy_after`) the egress
//! changes (if it can), at `rotate_fingerprint_after` the browser profile changes.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::detect::{ChallengeKind, Detection};
use seleniumbase_rs::sb_cdp::SolveOptions;
use seleniumbase_rs::utilities::retry::RetryPolicy;

/// How many attempts at one URL the default policy makes before giving up.
pub const DEFAULT_MAX_CHALLENGE_ATTEMPTS: u32 = 3;

/// How long the default policy's solve options wait for a result, and the browser leg for the
/// clearance after a solve: core's `SolveOptions::verify_timeout`, read back with
/// `verify_timeout_allowed`.
pub const DEFAULT_VERIFY_TIMEOUT: Duration = Duration::from_secs(30);

/// Challenges in a row after which the default policy changes the egress.
pub const DEFAULT_ROTATE_EGRESS_AFTER: u32 = 2;

/// Challenges in a row after which the default policy changes the browser profile.
pub const DEFAULT_ROTATE_PROFILE_AFTER: u32 = 3;

/// Seconds the default policy leaves a site alone after an outright block.
pub const DEFAULT_BLOCK_COOLDOWN_SECS: u64 = 900;

/// Seconds to leave a site alone after a rate limit that named no `Retry-After`.
pub const DEFAULT_RATE_LIMIT_COOLDOWN_SECS: u64 = 60;

/// The longest `Retry-After` honored, in seconds. A server cannot park a scraper for longer.
pub const MAX_RETRY_AFTER_SECS: u64 = 3600;

/// How a request is being made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FetchKind {
    /// Through a real browser.
    Browser,
    /// Through plain HTTP requests with an impersonated client.
    Http,
}

/// Where the fetch stands when a detection comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptContext {
    /// The attempt that produced the detection, counting from one.
    pub attempt: u32,
    /// How the request was made.
    pub fetch_kind: FetchKind,
    /// Challenges the site has shown in a row, this one included.
    pub consecutive_challenges: u32,
    /// Whether another egress is available.
    pub can_rotate_proxy: bool,
    /// Whether a browser is available to take over from [`FetchKind::Http`].
    pub can_escalate: bool,
    /// The time now, in seconds since the Unix epoch.
    pub now: u64,
}

/// What to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Mitigation {
    /// The response is real; use it.
    Proceed,
    /// Try again after the wait.
    Retry {
        /// How long to wait first.
        after: Duration,
    },
    /// Hand the request to the browser.
    Escalate,
    /// Let the browser solve the challenge and wait for the clearance.
    Solve,
    /// Leave through another egress.
    RotateProxy,
    /// Present a new browser profile.
    RotateFingerprint,
    /// Leave the site alone until the time given, in seconds since the Unix epoch.
    Cooldown {
        /// When the site may be tried again.
        until: u64,
    },
    /// Stop; the request failed.
    GiveUp(GiveUpReason),
}

/// Why the policy stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GiveUpReason {
    /// The attempts allowed are used.
    AttemptsExhausted,
    /// The attempts allowed are used, and the last answer was an outright block.
    Blocked,
    /// Only a different egress could help, and there is none.
    NoEgressLeft,
}

/// The thresholds of the decision table.
#[derive(Clone, Debug, PartialEq)]
pub struct MitigationPolicy {
    /// Attempts at one URL before giving up. Zero is read as one.
    pub max_challenge_attempts: u32,
    /// The wait before a retry that has no better reason to wait.
    pub backoff: RetryPolicy,
    /// How a challenge in the browser is solved, as core's `Page::solve_captcha_with` takes it
    /// (rounds, cooldown, clearance cookie, ...), and the rotation thresholds. Core's solver only
    /// carries `rotate_proxy_after` and `rotate_fingerprint_after`; this policy applies them to
    /// the challenges in a row (`proxy_rotation_due`, `fingerprint_rotation_due`).
    pub solve: SolveOptions,

    /// Seconds to leave a site alone after an outright block.
    pub block_cooldown_secs: u64,
}

impl Default for MitigationPolicy {
    fn default() -> Self {
        Self {
            max_challenge_attempts: DEFAULT_MAX_CHALLENGE_ATTEMPTS,
            backoff: RetryPolicy::network(),
            solve: SolveOptions::new()
                .verify_timeout(DEFAULT_VERIFY_TIMEOUT)
                .rotate_proxy_after(DEFAULT_ROTATE_EGRESS_AFTER)
                .rotate_fingerprint_after(DEFAULT_ROTATE_PROFILE_AFTER),

            block_cooldown_secs: DEFAULT_BLOCK_COOLDOWN_SECS,
        }
    }
}

impl MitigationPolicy {
    /// Decides the next step for `detection` in `context`. See the module documentation for the
    /// table.
    #[must_use]
    pub fn decide(&self, detection: &Detection, context: &AttemptContext) -> Mitigation {
        let kind = detection.kind;
        if kind == ChallengeKind::Clear {
            return Mitigation::Proceed;
        }
        if context.attempt >= self.max_challenge_attempts.max(1) {
            return Mitigation::GiveUp(if kind == ChallengeKind::Blocked {
                GiveUpReason::Blocked
            } else {
                GiveUpReason::AttemptsExhausted
            });
        }
        match kind {
            ChallengeKind::Clear => Mitigation::Proceed,
            ChallengeKind::RateLimited { retry_after_secs } => {
                let wait = retry_after_secs.map_or(DEFAULT_RATE_LIMIT_COOLDOWN_SECS, |s| {
                    s.min(MAX_RETRY_AFTER_SECS)
                });
                Mitigation::Cooldown {
                    until: context.now.saturating_add(wait),
                }
            }
            ChallengeKind::Blocked => {
                if context.can_rotate_proxy {
                    Mitigation::RotateProxy
                } else {
                    Mitigation::Cooldown {
                        until: context.now.saturating_add(self.block_cooldown_secs),
                    }
                }
            }
            ChallengeKind::JsChallenge | ChallengeKind::Interactive => self.challenged(context),
        }
    }

    /// The decision for a script or interactive challenge.
    fn challenged(&self, context: &AttemptContext) -> Mitigation {
        let streak = context.consecutive_challenges;
        match context.fetch_kind {
            FetchKind::Http if context.can_escalate => Mitigation::Escalate,
            FetchKind::Http => {
                if self.solve.fingerprint_rotation_due(streak) {
                    Mitigation::RotateFingerprint
                } else if self.solve.proxy_rotation_due(streak) {
                    if context.can_rotate_proxy {
                        Mitigation::RotateProxy
                    } else {
                        Mitigation::GiveUp(GiveUpReason::NoEgressLeft)
                    }
                } else {
                    Mitigation::Retry {
                        after: self.retry_delay(context.attempt),
                    }
                }
            }
            FetchKind::Browser => {
                if self.solve.fingerprint_rotation_due(streak) {
                    Mitigation::RotateFingerprint
                } else if self.solve.proxy_rotation_due(streak) && context.can_rotate_proxy {
                    Mitigation::RotateProxy
                } else {
                    Mitigation::Solve
                }
            }
        }
    }

    /// The wait before the attempt after `attempt`.
    fn retry_delay(&self, attempt: u32) -> Duration {
        let next = usize::try_from(attempt.saturating_add(1)).unwrap_or(usize::MAX);
        self.backoff.delay_for_attempt(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::challenge::detect::ChallengeEvidence;

    const NOW: u64 = 1_000_000;

    fn detection(kind: ChallengeKind) -> Detection {
        Detection {
            kind,
            evidence: ChallengeEvidence::default(),
        }
    }

    fn context(mode: FetchKind) -> AttemptContext {
        AttemptContext {
            attempt: 1,
            fetch_kind: mode,
            consecutive_challenges: 1,
            can_rotate_proxy: true,
            can_escalate: true,
            now: NOW,
        }
    }

    fn decide(kind: ChallengeKind, context: &AttemptContext) -> Mitigation {
        MitigationPolicy::default().decide(&detection(kind), context)
    }

    const JS: ChallengeKind = ChallengeKind::JsChallenge;

    #[test]
    fn the_defaults_are_the_named_constants() {
        let policy = MitigationPolicy::default();
        assert_eq!(policy.max_challenge_attempts, 3);
        assert!(policy.solve.proxy_rotation_due(2) && !policy.solve.proxy_rotation_due(1));
        assert!(
            policy.solve.fingerprint_rotation_due(3) && !policy.solve.fingerprint_rotation_due(2)
        );
        assert_eq!(policy.block_cooldown_secs, 900);
        assert_eq!(DEFAULT_VERIFY_TIMEOUT, Duration::from_secs(30));
        assert_eq!(
            policy.solve.verify_timeout_allowed(),
            DEFAULT_VERIFY_TIMEOUT
        );

        assert_eq!(policy.backoff, RetryPolicy::network());
        assert_eq!(
            policy.max_challenge_attempts,
            DEFAULT_MAX_CHALLENGE_ATTEMPTS
        );
        assert_eq!(policy.block_cooldown_secs, DEFAULT_BLOCK_COOLDOWN_SECS);
    }

    #[test]
    fn a_page_that_is_not_a_challenge_proceeds_whatever_the_attempt() {
        for attempt in [0, 1, 3, 100, u32::MAX] {
            let ctx = AttemptContext {
                attempt,
                ..context(FetchKind::Http)
            };
            assert_eq!(decide(ChallengeKind::Clear, &ctx), Mitigation::Proceed);
        }
    }

    #[test]
    fn a_script_challenge_in_http_mode_escalates_to_the_browser() {
        assert_eq!(decide(JS, &context(FetchKind::Http)), Mitigation::Escalate);
        assert_eq!(
            decide(ChallengeKind::Interactive, &context(FetchKind::Http)),
            Mitigation::Escalate
        );
    }

    #[test]
    fn a_challenge_in_the_browser_is_solved_first() {
        let ctx = AttemptContext {
            consecutive_challenges: 1,
            ..context(FetchKind::Browser)
        };
        assert_eq!(decide(JS, &ctx), Mitigation::Solve);
        assert_eq!(decide(ChallengeKind::Interactive, &ctx), Mitigation::Solve);
        let first = AttemptContext {
            consecutive_challenges: 0,
            ..ctx
        };
        assert_eq!(decide(JS, &first), Mitigation::Solve);
    }

    #[test]
    fn the_browser_ladder_rises_with_the_streak() {
        let at = |streak, can_rotate_proxy| AttemptContext {
            consecutive_challenges: streak,
            can_rotate_proxy,
            ..context(FetchKind::Browser)
        };
        assert_eq!(decide(JS, &at(2, true)), Mitigation::RotateProxy);
        assert_eq!(
            decide(JS, &at(2, false)),
            Mitigation::Solve,
            "nothing to rotate to"
        );
        assert_eq!(decide(JS, &at(3, true)), Mitigation::RotateFingerprint);
        assert_eq!(decide(JS, &at(3, false)), Mitigation::RotateFingerprint);
        assert_eq!(decide(JS, &at(40, true)), Mitigation::RotateFingerprint);
        assert_eq!(
            decide(JS, &at(u32::MAX, true)),
            Mitigation::RotateFingerprint
        );
    }

    #[test]
    fn http_without_a_browser_retries_then_rotates_then_has_nothing_left() {
        let at = |streak, can_rotate_proxy| AttemptContext {
            consecutive_challenges: streak,
            can_rotate_proxy,
            can_escalate: false,
            ..context(FetchKind::Http)
        };
        assert_eq!(
            decide(JS, &at(1, true)),
            Mitigation::Retry {
                after: Duration::from_millis(500)
            },
            "the first retry waits the base delay of the network policy"
        );
        assert_eq!(decide(JS, &at(2, true)), Mitigation::RotateProxy);
        assert_eq!(
            decide(JS, &at(2, false)),
            Mitigation::GiveUp(GiveUpReason::NoEgressLeft)
        );
        assert_eq!(decide(JS, &at(3, false)), Mitigation::RotateFingerprint);
    }

    #[test]
    fn the_retry_wait_grows_with_the_attempt_and_stops_at_the_cap() {
        let policy = MitigationPolicy {
            max_challenge_attempts: 10_000,
            ..MitigationPolicy::default()
        };
        let wait = |attempt| {
            let ctx = AttemptContext {
                attempt,
                consecutive_challenges: 1,
                can_escalate: false,
                ..context(FetchKind::Http)
            };
            match policy.decide(&detection(JS), &ctx) {
                Mitigation::Retry { after } => after,
                other => panic!("expected a retry, got {other:?}"),
            }
        };
        assert_eq!(wait(1), Duration::from_millis(500));
        assert_eq!(wait(2), Duration::from_secs(1));
        assert_eq!(wait(3), Duration::from_secs(2));
        assert_eq!(
            wait(60),
            Duration::from_secs(10),
            "the cap of the network policy"
        );
        assert_eq!(wait(5_000), Duration::from_secs(10));
    }

    #[test]
    fn a_block_rotates_the_egress_or_leaves_the_site_alone() {
        let rotate = context(FetchKind::Http);
        assert_eq!(
            decide(ChallengeKind::Blocked, &rotate),
            Mitigation::RotateProxy
        );
        let stuck = AttemptContext {
            can_rotate_proxy: false,
            ..rotate
        };
        assert_eq!(
            decide(ChallengeKind::Blocked, &stuck),
            Mitigation::Cooldown { until: NOW + 900 }
        );
        let in_browser = AttemptContext {
            fetch_kind: FetchKind::Browser,
            ..stuck
        };
        assert_eq!(
            decide(ChallengeKind::Blocked, &in_browser),
            Mitigation::Cooldown { until: NOW + 900 }
        );
    }

    #[test]
    fn a_rate_limit_waits_for_retry_after_but_never_for_long() {
        let limited = |secs| ChallengeKind::RateLimited {
            retry_after_secs: secs,
        };
        let ctx = context(FetchKind::Http);
        assert_eq!(
            decide(limited(Some(30)), &ctx),
            Mitigation::Cooldown { until: NOW + 30 }
        );
        assert_eq!(
            decide(limited(Some(0)), &ctx),
            Mitigation::Cooldown { until: NOW }
        );
        assert_eq!(
            decide(limited(None), &ctx),
            Mitigation::Cooldown {
                until: NOW + DEFAULT_RATE_LIMIT_COOLDOWN_SECS
            }
        );
        assert_eq!(
            decide(limited(Some(MAX_RETRY_AFTER_SECS + 1)), &ctx),
            Mitigation::Cooldown {
                until: NOW + MAX_RETRY_AFTER_SECS
            }
        );
        assert_eq!(
            decide(limited(Some(u64::MAX)), &ctx),
            Mitigation::Cooldown {
                until: NOW + MAX_RETRY_AFTER_SECS
            }
        );
    }

    #[test]
    fn a_cooldown_saturates_instead_of_wrapping() {
        let ctx = AttemptContext {
            now: u64::MAX - 5,
            ..context(FetchKind::Http)
        };
        assert_eq!(
            decide(
                ChallengeKind::RateLimited {
                    retry_after_secs: Some(100)
                },
                &ctx
            ),
            Mitigation::Cooldown { until: u64::MAX }
        );
    }

    #[test]
    fn the_attempts_run_out() {
        for attempt in [3, 4, 1000, u32::MAX] {
            let ctx = AttemptContext {
                attempt,
                ..context(FetchKind::Browser)
            };
            assert_eq!(
                decide(JS, &ctx),
                Mitigation::GiveUp(GiveUpReason::AttemptsExhausted)
            );
            assert_eq!(
                decide(ChallengeKind::Interactive, &ctx),
                Mitigation::GiveUp(GiveUpReason::AttemptsExhausted)
            );
            assert_eq!(
                decide(
                    ChallengeKind::RateLimited {
                        retry_after_secs: Some(1)
                    },
                    &ctx
                ),
                Mitigation::GiveUp(GiveUpReason::AttemptsExhausted)
            );
            assert_eq!(
                decide(ChallengeKind::Blocked, &ctx),
                Mitigation::GiveUp(GiveUpReason::Blocked)
            );
        }
        let last = AttemptContext {
            attempt: 2,
            ..context(FetchKind::Browser)
        };
        assert_eq!(
            decide(JS, &last),
            Mitigation::Solve,
            "the third attempt is still allowed"
        );
    }

    #[test]
    fn zero_attempts_allowed_is_read_as_one() {
        let policy = MitigationPolicy {
            max_challenge_attempts: 0,
            ..MitigationPolicy::default()
        };
        let ctx = context(FetchKind::Browser);
        assert_eq!(
            policy.decide(&detection(JS), &ctx),
            Mitigation::GiveUp(GiveUpReason::AttemptsExhausted)
        );
        assert_eq!(
            policy.decide(&detection(ChallengeKind::Clear), &ctx),
            Mitigation::Proceed
        );
    }

    #[test]
    fn the_thresholds_are_the_policys_own() {
        let policy = MitigationPolicy {
            solve: SolveOptions::new()
                .rotate_proxy_after(1)
                .rotate_fingerprint_after(5),

            block_cooldown_secs: 10,
            ..MitigationPolicy::default()
        };
        let at = |streak| AttemptContext {
            consecutive_challenges: streak,
            ..context(FetchKind::Browser)
        };
        assert_eq!(
            policy.decide(&detection(JS), &at(1)),
            Mitigation::RotateProxy
        );
        assert_eq!(
            policy.decide(&detection(JS), &at(4)),
            Mitigation::RotateProxy
        );
        assert_eq!(
            policy.decide(&detection(JS), &at(5)),
            Mitigation::RotateFingerprint
        );
        let stuck = AttemptContext {
            can_rotate_proxy: false,
            ..context(FetchKind::Browser)
        };
        assert_eq!(
            policy.decide(&detection(ChallengeKind::Blocked), &stuck),
            Mitigation::Cooldown { until: NOW + 10 }
        );
    }

    #[test]
    fn evidence_does_not_change_the_decision() {
        let with_evidence = Detection {
            kind: JS,
            evidence: ChallengeEvidence {
                vendor: Some(crate::challenge::detect::ProtectionVendor::Cloudflare),
                markers: vec!["cf_chl_opt"],
                widget: None,
            },
        };
        let ctx = context(FetchKind::Http);
        assert_eq!(
            MitigationPolicy::default().decide(&with_evidence, &ctx),
            decide(JS, &ctx)
        );
    }

    #[test]
    fn every_input_gets_an_answer_that_keeps_its_promises() {
        let kinds = [
            ChallengeKind::Clear,
            JS,
            ChallengeKind::Interactive,
            ChallengeKind::Blocked,
            ChallengeKind::RateLimited {
                retry_after_secs: None,
            },
            ChallengeKind::RateLimited {
                retry_after_secs: Some(u64::MAX),
            },
        ];
        let policy = MitigationPolicy::default();
        for kind in kinds {
            for mode in [FetchKind::Browser, FetchKind::Http] {
                for (can_rotate_proxy, can_escalate) in
                    [(false, false), (false, true), (true, false), (true, true)]
                {
                    for attempt in 0..6 {
                        for streak in 0..6 {
                            let ctx = AttemptContext {
                                attempt,
                                fetch_kind: mode,
                                consecutive_challenges: streak,
                                can_rotate_proxy,
                                can_escalate,
                                now: NOW,
                            };
                            let action = policy.decide(&detection(kind), &ctx);
                            let note = format!("{kind:?} {ctx:?} -> {action:?}");
                            assert_eq!(
                                action == Mitigation::Proceed,
                                kind == ChallengeKind::Clear,
                                "{note}"
                            );
                            if action == Mitigation::RotateProxy {
                                assert!(can_rotate_proxy, "{note}");
                            }
                            if action == Mitigation::Escalate {
                                assert!(can_escalate && mode == FetchKind::Http, "{note}");
                            }
                            if action == Mitigation::Solve {
                                assert_eq!(mode, FetchKind::Browser, "{note}");
                            }
                            if attempt >= 3 && kind != ChallengeKind::Clear {
                                assert!(matches!(action, Mitigation::GiveUp(_)), "{note}");
                            }
                            if let Mitigation::Cooldown { until } = action {
                                assert!(until >= NOW, "{note}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn actions_and_modes_survive_serde() {
        let actions = [
            Mitigation::Proceed,
            Mitigation::Retry {
                after: Duration::from_millis(1500),
            },
            Mitigation::Escalate,
            Mitigation::Solve,
            Mitigation::RotateProxy,
            Mitigation::RotateFingerprint,
            Mitigation::Cooldown { until: 42 },
            Mitigation::GiveUp(GiveUpReason::NoEgressLeft),
        ];
        for action in actions {
            let json = serde_json::to_string(&action).expect("serializes");
            assert_eq!(
                serde_json::from_str::<Mitigation>(&json).expect("parses"),
                action
            );
        }
        for mode in [FetchKind::Browser, FetchKind::Http] {
            let json = serde_json::to_string(&mode).expect("serializes");
            assert_eq!(
                serde_json::from_str::<FetchKind>(&json).expect("parses"),
                mode
            );
        }
    }
}
