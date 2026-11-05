//! Decides when a session moves between browser and HTTP mode, and when a clearance is near expiry.
//!
//! The policy is a pure function of the mode, the identity, the site's state and the time
//! (`now` is passed in; nothing here reads a clock). A session sits in [`FetchKind::Http`] while it
//! can and goes to [`FetchKind::Browser`] when HTTP is not enough. It asks twice per fetch.
//!
//! # Before a fetch ([`EscalationPolicy::before_fetch`])
//!
//! Only an HTTP session ever leaves here; a browser session stays where it is.
//!
//! | FetchKind | The identity holds | Other | Escalation |
//! |---|---|---|---|
//! | Browser | anything | anything | [`Escalation::Stay`] |
//! | Http | a clearance with at most `clearance_margin` left, or an expired one | anything | escalate: [`EscalateReason::ClearanceExpiring`] |
//! | Http | a clearance with more than the margin left | anything | [`Escalation::Stay`], even if the site needs a browser |
//! | Http | no clearance for the site | `require_clearance_for_http` | escalate: [`EscalateReason::NoClearance`] |
//! | Http | no clearance for the site | the site's `preferred_kind` is the browser | escalate: [`EscalateReason::SiteNeedsBrowser`] |
//! | Http | no clearance for the site | otherwise | [`Escalation::Stay`] |
//!
//! A clearance that is held and valid wins over the site's history, because earning it in a
//! browser and then carrying it over HTTP is the point of leaving the browser. A cooldown is not
//! the policy's concern: the caller checks it before it fetches at all.
//!
//! # After a fetch ([`EscalationPolicy::after_fetch`])
//!
//! `action` is what the mitigation policy decided for `detection`, and it has just been carried
//! out. For [`Mitigation::Solve`] that means the challenge was solved. Call
//! [`Identity::refresh_clearances`] first so the identity shows what the solve earned.
//!
//! | FetchKind | Mitigation | Other | Escalation |
//! |---|---|---|---|
//! | Http | [`Mitigation::Escalate`] | anything | escalate: [`EscalateReason::Challenge`] with the detected kind |
//! | Browser | [`Mitigation::Solve`] | `demote_after_solve`, `prefer` is HTTP, the identity holds a clearance | demote: [`DemoteReason::ChallengeSolved`] |
//! | any | anything else | anything | [`Escalation::Stay`] |
//!
//! [`EscalateReason::Manual`] and [`DemoteReason::Manual`] are never produced here; a session
//! uses them when its caller asks for a change.

use std::time::Duration;

use serde::Serialize;

use super::identity::Identity;
use crate::challenge::detect::{ChallengeKind, Detection};
use crate::challenge::mitigation::{FetchKind, Mitigation};
use crate::ledger::state::SiteRecord;

#[cfg(test)]
mod tests;

/// How close to its end a clearance may get before an HTTP session goes back to the browser.
pub const DEFAULT_CLEARANCE_MARGIN_SECS: u64 = 120;

/// When a session changes mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EscalationPolicy {
    /// An HTTP request needs a clearance for its site; without one the session escalates.
    pub require_clearance_for_http: bool,
    /// A clearance with at most this long left (whole seconds) counts as expiring.
    pub clearance_margin: Duration,
    /// Go back to HTTP once the browser has solved a challenge and the identity holds a clearance.
    pub demote_after_solve: bool,
    /// The mode a new session starts in and returns to.
    pub prefer: FetchKind,
}

impl Default for EscalationPolicy {
    /// No clearance required, a margin of [`DEFAULT_CLEARANCE_MARGIN_SECS`], demote after a
    /// solve, and prefer HTTP.
    fn default() -> Self {
        Self {
            require_clearance_for_http: false,
            clearance_margin: Duration::from_secs(DEFAULT_CLEARANCE_MARGIN_SECS),
            demote_after_solve: true,
            prefer: FetchKind::Http,
        }
    }
}

/// A change of mode, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Escalation {
    /// Keep the mode.
    Stay,
    /// Go from HTTP to the browser.
    Escalate(EscalateReason),
    /// Go from the browser back to HTTP.
    Demote(DemoteReason),
}

/// Why a session goes to the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EscalateReason {
    /// A fetch met this challenge.
    Challenge(ChallengeKind),
    /// The clearance for the site ends soon, or has ended.
    ClearanceExpiring {
        /// Seconds left; zero once it has ended.
        remaining_secs: u64,
    },
    /// The policy requires a clearance and the identity has none for the site.
    NoClearance,
    /// The site's history says plain requests do not work.
    SiteNeedsBrowser,
    /// The caller asked for it.
    Manual,
}

/// Why a session goes back to HTTP.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DemoteReason {
    /// The browser solved a challenge and the identity holds the clearance.
    ChallengeSolved,
    /// The caller asked for it.
    Manual,
}

impl Escalation {
    /// The mode a session in `current` is in after this transition.
    #[must_use]
    pub fn resulting_kind(self, current: FetchKind) -> FetchKind {
        match self {
            Self::Stay => current,
            Self::Escalate(_) => FetchKind::Browser,
            Self::Demote(_) => FetchKind::Http,
        }
    }
}

impl EscalationPolicy {
    /// Whether to change mode before fetching `site`. See the module documentation for the table.
    #[must_use]
    pub fn before_fetch(
        &self,
        mode: FetchKind,
        id: &Identity,
        site: &str,
        st: Option<&SiteRecord>,
        now: u64,
    ) -> Escalation {
        match mode {
            FetchKind::Browser => Escalation::Stay,
            FetchKind::Http => self.http_before_fetch(id, site, st, now),
        }
    }

    /// Whether to change mode after the action chosen for `d` has been carried out. See the
    /// module documentation for the table.
    #[must_use]
    pub fn after_fetch(
        &self,
        mode: FetchKind,
        d: &Detection,
        a: &Mitigation,
        id: &Identity,
    ) -> Escalation {
        match (mode, a) {
            (FetchKind::Http, Mitigation::Escalate) => {
                Escalation::Escalate(EscalateReason::Challenge(d.kind))
            }
            (FetchKind::Browser, Mitigation::Solve) if self.demotes_after_solve(id) => {
                Escalation::Demote(DemoteReason::ChallengeSolved)
            }
            _ => Escalation::Stay,
        }
    }

    /// The rows of the table for an HTTP session before a fetch.
    fn http_before_fetch(
        &self,
        id: &Identity,
        site: &str,
        st: Option<&SiteRecord>,
        now: u64,
    ) -> Escalation {
        if let Some(clearance) = id.clearance_for(site) {
            let remaining_secs = clearance.expires_at.saturating_sub(now);
            return if remaining_secs <= self.clearance_margin.as_secs() {
                Escalation::Escalate(EscalateReason::ClearanceExpiring { remaining_secs })
            } else {
                Escalation::Stay
            };
        }
        if self.require_clearance_for_http {
            return Escalation::Escalate(EscalateReason::NoClearance);
        }
        if st.is_some_and(|state| state.preferred_kind == Some(FetchKind::Browser)) {
            return Escalation::Escalate(EscalateReason::SiteNeedsBrowser);
        }
        Escalation::Stay
    }

    /// Whether a browser session that has just solved a challenge goes back to HTTP.
    fn demotes_after_solve(&self, id: &Identity) -> bool {
        self.demote_after_solve && self.prefer == FetchKind::Http && !id.clearances.is_empty()
    }
}
