//! Tests of the session policy: every row of both tables, and the edges of the margin.

use super::*;
use crate::challenge::detect::{ChallengeEvidence, ProtectionVendor};
use crate::challenge::mitigation::GiveUpReason;
use crate::passport::identity::fixtures::{identity, identity_with_clearance, NOW};
use crate::passport::identity::{Clearance, NO_EXPIRY};

const SITE: &str = "example.com";

/// An identity holding a clearance for [`SITE`] that ends at `expires_at`.
fn cleared_until(expires_at: u64) -> Identity {
    let mut made = identity();
    made.clearances.push(Clearance {
        site: SITE.to_owned(),
        vendor: ProtectionVendor::Cloudflare,
        cookie: "cf_clearance".to_owned(),
        expires_at,
    });
    made
}

/// The state of a site whose history prefers `mode`.
fn site_preferring(mode: FetchKind) -> SiteRecord {
    SiteRecord {
        preferred_kind: Some(mode),
        ..SiteRecord::new(SITE)
    }
}

fn seen(kind: ChallengeKind) -> Detection {
    Detection {
        kind,
        evidence: ChallengeEvidence::default(),
    }
}

fn before(
    policy: &EscalationPolicy,
    mode: FetchKind,
    id: &Identity,
    st: Option<&SiteRecord>,
) -> Escalation {
    policy.before_fetch(mode, id, SITE, st, NOW)
}

fn expiring(remaining_secs: u64) -> Escalation {
    Escalation::Escalate(EscalateReason::ClearanceExpiring { remaining_secs })
}

#[test]
fn the_defaults_are_the_documented_ones() {
    let policy = EscalationPolicy::default();
    assert!(!policy.require_clearance_for_http);
    assert_eq!(policy.clearance_margin, Duration::from_secs(120));
    assert_eq!(
        policy.clearance_margin.as_secs(),
        DEFAULT_CLEARANCE_MARGIN_SECS
    );
    assert!(policy.demote_after_solve);
    assert_eq!(policy.prefer, FetchKind::Http);
}

#[test]
fn an_http_session_without_history_or_clearance_stays() {
    let policy = EscalationPolicy::default();
    assert_eq!(
        before(&policy, FetchKind::Http, &identity(), None),
        Escalation::Stay
    );
    let fresh = SiteRecord::new(SITE);
    assert_eq!(
        before(&policy, FetchKind::Http, &identity(), Some(&fresh)),
        Escalation::Stay
    );
}

#[test]
fn a_required_clearance_that_is_missing_escalates() {
    let policy = EscalationPolicy {
        require_clearance_for_http: true,
        ..EscalationPolicy::default()
    };
    assert_eq!(
        before(&policy, FetchKind::Http, &identity(), None),
        Escalation::Escalate(EscalateReason::NoClearance)
    );
}

#[test]
fn a_clearance_for_another_site_does_not_count() {
    let policy = EscalationPolicy {
        require_clearance_for_http: true,
        ..EscalationPolicy::default()
    };
    assert_eq!(
        policy.before_fetch(
            FetchKind::Http,
            &cleared_until(NOW + 1000),
            "other.test",
            None,
            NOW
        ),
        Escalation::Escalate(EscalateReason::NoClearance)
    );
}

#[test]
fn a_site_that_needs_a_browser_escalates() {
    let policy = EscalationPolicy::default();
    let needs_browser = site_preferring(FetchKind::Browser);
    assert_eq!(
        before(&policy, FetchKind::Http, &identity(), Some(&needs_browser)),
        Escalation::Escalate(EscalateReason::SiteNeedsBrowser)
    );
    let works_over_http = site_preferring(FetchKind::Http);
    assert_eq!(
        before(
            &policy,
            FetchKind::Http,
            &identity(),
            Some(&works_over_http)
        ),
        Escalation::Stay
    );
}

#[test]
fn a_missing_required_clearance_is_named_before_the_site_history() {
    let policy = EscalationPolicy {
        require_clearance_for_http: true,
        ..EscalationPolicy::default()
    };
    let needs_browser = site_preferring(FetchKind::Browser);
    assert_eq!(
        before(&policy, FetchKind::Http, &identity(), Some(&needs_browser)),
        Escalation::Escalate(EscalateReason::NoClearance)
    );
}

#[test]
fn a_valid_clearance_keeps_an_http_session_even_when_the_site_needs_a_browser() {
    for require in [false, true] {
        let policy = EscalationPolicy {
            require_clearance_for_http: require,
            ..EscalationPolicy::default()
        };
        let needs_browser = site_preferring(FetchKind::Browser);
        assert_eq!(
            before(
                &policy,
                FetchKind::Http,
                &cleared_until(NOW + 1000),
                Some(&needs_browser)
            ),
            Escalation::Stay,
            "require = {require}"
        );
    }
}

#[test]
fn the_real_clearance_of_a_jar_keeps_an_http_session() {
    let policy = EscalationPolicy::default();
    let made = identity_with_clearance();
    assert_eq!(
        before(&policy, FetchKind::Http, &made, None),
        Escalation::Stay
    );
    // An hour from NOW the jar's cookie ends; 100 s before that it is expiring.
    assert_eq!(
        policy.before_fetch(FetchKind::Http, &made, "www.example.com", None, NOW + 3500),
        expiring(100)
    );
}

#[test]
fn the_margin_is_inclusive_at_120_seconds() {
    let policy = EscalationPolicy::default();
    let at = |remaining: u64| {
        before(
            &policy,
            FetchKind::Http,
            &cleared_until(NOW + remaining),
            None,
        )
    };
    assert_eq!(at(121), Escalation::Stay);
    assert_eq!(at(120), expiring(120));
    assert_eq!(at(119), expiring(119));
    assert_eq!(at(1), expiring(1));
}

#[test]
fn an_expired_clearance_escalates_with_nothing_left() {
    let policy = EscalationPolicy::default();
    assert_eq!(
        before(&policy, FetchKind::Http, &cleared_until(NOW), None),
        expiring(0),
        "it ends exactly now"
    );
    assert_eq!(
        before(&policy, FetchKind::Http, &cleared_until(NOW - 5), None),
        expiring(0)
    );
    assert_eq!(
        before(&policy, FetchKind::Http, &cleared_until(0), None),
        expiring(0)
    );
}

#[test]
fn a_zero_margin_escalates_only_once_the_clearance_has_ended() {
    let policy = EscalationPolicy {
        clearance_margin: Duration::ZERO,
        ..EscalationPolicy::default()
    };
    assert_eq!(
        before(&policy, FetchKind::Http, &cleared_until(NOW + 1), None),
        Escalation::Stay
    );
    assert_eq!(
        before(&policy, FetchKind::Http, &cleared_until(NOW), None),
        expiring(0)
    );
}

#[test]
fn a_longer_margin_escalates_sooner_and_sub_seconds_are_dropped() {
    let long = EscalationPolicy {
        clearance_margin: Duration::from_secs(600),
        ..EscalationPolicy::default()
    };
    assert_eq!(
        before(&long, FetchKind::Http, &cleared_until(NOW + 500), None),
        expiring(500)
    );
    let fractional = EscalationPolicy {
        clearance_margin: Duration::from_millis(1999),
        ..EscalationPolicy::default()
    };
    assert_eq!(
        before(&fractional, FetchKind::Http, &cleared_until(NOW + 2), None),
        Escalation::Stay,
        "1999 ms is read as one second"
    );
    assert_eq!(
        before(&fractional, FetchKind::Http, &cleared_until(NOW + 1), None),
        expiring(1)
    );
}

#[test]
fn a_clearance_with_no_known_end_never_expires() {
    let policy = EscalationPolicy::default();
    assert_eq!(
        before(&policy, FetchKind::Http, &cleared_until(NO_EXPIRY), None),
        Escalation::Stay
    );
    assert_eq!(
        policy.before_fetch(
            FetchKind::Http,
            &cleared_until(NO_EXPIRY),
            SITE,
            None,
            u64::MAX
        ),
        expiring(0),
        "even a session cookie is spent at the end of time"
    );
}

#[test]
fn a_browser_session_never_leaves_before_a_fetch() {
    let policies = [
        EscalationPolicy::default(),
        EscalationPolicy {
            require_clearance_for_http: true,
            ..EscalationPolicy::default()
        },
    ];
    let needs_browser = site_preferring(FetchKind::Browser);
    for policy in policies {
        for id in [identity(), cleared_until(NOW + 1000), cleared_until(NOW)] {
            assert_eq!(
                before(&policy, FetchKind::Browser, &id, Some(&needs_browser)),
                Escalation::Stay
            );
        }
    }
}

#[test]
fn a_cooldown_is_left_to_the_caller() {
    let mut cooling = SiteRecord::new(SITE);
    cooling.start_cooldown(NOW + 1000);
    assert!(cooling.in_cooldown(NOW));
    assert_eq!(
        before(
            &EscalationPolicy::default(),
            FetchKind::Http,
            &identity(),
            Some(&cooling)
        ),
        Escalation::Stay
    );
}

#[test]
fn a_challenge_escalates_an_http_session_with_the_kind_met() {
    let policy = EscalationPolicy::default();
    for kind in [
        ChallengeKind::JsChallenge,
        ChallengeKind::Interactive,
        ChallengeKind::Blocked,
    ] {
        assert_eq!(
            policy.after_fetch(
                FetchKind::Http,
                &seen(kind),
                &Mitigation::Escalate,
                &identity()
            ),
            Escalation::Escalate(EscalateReason::Challenge(kind))
        );
    }
}

#[test]
fn every_other_action_leaves_an_http_session_alone() {
    let policy = EscalationPolicy::default();
    let actions = [
        Mitigation::Proceed,
        Mitigation::Retry {
            after: Duration::from_secs(1),
        },
        Mitigation::Solve,
        Mitigation::RotateProxy,
        Mitigation::RotateFingerprint,
        Mitigation::Cooldown { until: NOW + 60 },
        Mitigation::GiveUp(GiveUpReason::AttemptsExhausted),
    ];
    for action in actions {
        assert_eq!(
            policy.after_fetch(
                FetchKind::Http,
                &seen(ChallengeKind::JsChallenge),
                &action,
                &identity_with_clearance()
            ),
            Escalation::Stay,
            "{action:?}"
        );
    }
}

#[test]
fn a_solve_in_the_browser_demotes_when_a_clearance_was_earned() {
    let policy = EscalationPolicy::default();
    assert_eq!(
        policy.after_fetch(
            FetchKind::Browser,
            &seen(ChallengeKind::Interactive),
            &Mitigation::Solve,
            &identity_with_clearance()
        ),
        Escalation::Demote(DemoteReason::ChallengeSolved)
    );
}

#[test]
fn a_solve_does_not_demote_without_a_clearance_to_carry() {
    let policy = EscalationPolicy::default();
    assert_eq!(
        policy.after_fetch(
            FetchKind::Browser,
            &seen(ChallengeKind::Interactive),
            &Mitigation::Solve,
            &identity()
        ),
        Escalation::Stay
    );
}

#[test]
fn a_solve_does_not_demote_when_the_policy_says_not_to() {
    let keep_browser = EscalationPolicy {
        demote_after_solve: false,
        ..EscalationPolicy::default()
    };
    let prefers_browser = EscalationPolicy {
        prefer: FetchKind::Browser,
        ..EscalationPolicy::default()
    };
    for policy in [keep_browser, prefers_browser] {
        assert_eq!(
            policy.after_fetch(
                FetchKind::Browser,
                &seen(ChallengeKind::JsChallenge),
                &Mitigation::Solve,
                &identity_with_clearance()
            ),
            Escalation::Stay,
            "{policy:?}"
        );
    }
}

#[test]
fn other_actions_in_the_browser_stay() {
    let policy = EscalationPolicy::default();
    for action in [
        Mitigation::Proceed,
        Mitigation::Escalate,
        Mitigation::RotateProxy,
        Mitigation::RotateFingerprint,
        Mitigation::Cooldown { until: NOW },
        Mitigation::GiveUp(GiveUpReason::Blocked),
    ] {
        assert_eq!(
            policy.after_fetch(
                FetchKind::Browser,
                &seen(ChallengeKind::Clear),
                &action,
                &identity_with_clearance()
            ),
            Escalation::Stay,
            "{action:?}"
        );
    }
}

#[test]
fn a_transition_names_the_mode_it_leads_to() {
    let escalate = Escalation::Escalate(EscalateReason::Manual);
    let demote = Escalation::Demote(DemoteReason::Manual);
    assert_eq!(escalate.resulting_kind(FetchKind::Http), FetchKind::Browser);
    assert_eq!(demote.resulting_kind(FetchKind::Browser), FetchKind::Http);
    assert_eq!(
        Escalation::Stay.resulting_kind(FetchKind::Http),
        FetchKind::Http
    );
    assert_eq!(
        Escalation::Stay.resulting_kind(FetchKind::Browser),
        FetchKind::Browser
    );
}

#[test]
fn a_transition_serializes_as_snake_case_json() {
    let json = |transition: Escalation| serde_json::to_value(transition).expect("serializes");
    assert_eq!(json(Escalation::Stay), serde_json::json!("stay"));
    assert_eq!(
        json(expiring(5)),
        serde_json::json!({"escalate": {"clearance_expiring": {"remaining_secs": 5}}})
    );
    assert_eq!(
        json(Escalation::Escalate(EscalateReason::NoClearance)),
        serde_json::json!({"escalate": "no_clearance"})
    );
    assert_eq!(
        json(Escalation::Demote(DemoteReason::ChallengeSolved)),
        serde_json::json!({"demote": "challenge_solved"})
    );
}
