//! The proxy pool: core's `ProxyList` rotating over proxy routes, which add the exit country.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::geo::CountryCode;
use super::redact::RedactedUrl;
use super::route::{ProxyKey, ProxyRoute, ProxyRouteError};
use seleniumbase_rs::config::proxy_list::{ProxyEntry, ProxyList, ProxyOutcome, RotationStrategy};

/// Failures in a row that quarantine a proxy, unless the pool says otherwise.
pub const DEFAULT_QUARANTINE_AFTER: u32 = 3;

/// Seconds a quarantined proxy stays out of rotation, unless the pool says otherwise.
pub const DEFAULT_QUARANTINE_SECS: u64 = 600;

/// The last second the pool reads as a time: the end of the year 9999.
///
/// The caller gives the pool Unix seconds, and a time past this one is read as this one, so that
/// adding a quarantine to it cannot overflow the clock type of the platform. No proxy rests
/// longer than that.
const LAST_SECOND: u64 = 253_402_300_799;

/// A rotating set of proxy routes, on core's [`ProxyList`], each with the country it exits in.
///
/// Every decision takes the current time in seconds as an argument, so the pool itself reads no
/// clock. It is `Send` and `Sync`: share it behind an `Arc`.
pub struct ProxyPool {
    /// Core's list: the rotation, the failure counts and the quarantine are its.
    list: ProxyList,
    /// The routes, in the order of the list's entries.
    routes: Vec<ProxyRoute>,
}

impl fmt::Debug for ProxyPool {
    /// Counts the routes instead of printing them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProxyPool")
            .field("routes", &self.routes.len())
            .finish_non_exhaustive()
    }
}

/// The time `now` seconds after the Unix epoch, read as the last second of the calendar at most.
fn at(now: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(now.min(LAST_SECOND))
}

/// The address core's list holds for a route: its key as written, without the country. The list
/// knows `socks5` and not `socks5h`, and it needs the address only to keep the failures of each
/// proxy apart; the route itself keeps its own scheme.
fn list_address(key: &ProxyKey) -> String {
    let shown = key.to_string();
    match shown.strip_prefix("socks5h://") {
        Some(rest) => format!("socks5://{rest}"),
        None => shown,
    }
}

impl ProxyPool {
    /// A pool over `routes`, rotating the way `rotation` says (core's [`RotationStrategy`]), with
    /// the default quarantine: [`DEFAULT_QUARANTINE_AFTER`] failures in a row put a proxy out of
    /// rotation for [`DEFAULT_QUARANTINE_SECS`] seconds.
    ///
    /// # Errors
    ///
    /// Returns [`ProxyRouteError::Address`] for a route whose address core's [`ProxyList`] refuses,
    /// such as a socks host with a `%` in it. The error holds no credentials.
    pub fn from_routes(
        routes: Vec<ProxyRoute>,
        rotation: RotationStrategy,
    ) -> Result<Self, ProxyRouteError> {
        let mut list = ProxyList::new().with_rotation(rotation).with_quarantine(
            DEFAULT_QUARANTINE_AFTER,
            Duration::from_secs(DEFAULT_QUARANTINE_SECS),
        );
        for route in &routes {
            let address = list_address(route.key());
            list.add(&address)
                .map_err(|_error| ProxyRouteError::Address(RedactedUrl::new(&address)))?;
        }
        Ok(Self { list, routes })
    }

    /// A pool over the proxies of a core [`ProxyList`] (`--proxy-list`, `SB_PROXY_LIST`), kept as
    /// the list was set up: its rotation and its quarantine, if it has one, are the pool's. A list
    /// entry carries no country, so [`pick_in`](Self::pick_in) finds nothing in such a pool.
    ///
    /// # Errors
    ///
    /// Returns the [`ProxyRouteError`] of the first entry that is not a route: core's list takes
    /// `socks4`, a route does not.
    pub fn from_list(list: ProxyList) -> Result<Self, ProxyRouteError> {
        let routes = list
            .iter()
            .map(|entry| ProxyRoute::parse(entry.spec(), None))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { list, routes })
    }

    /// A pool built from `(spec, country)` pairs, each spec parsed the way [`ProxyRoute::parse`]
    /// parses it.
    ///
    /// # Errors
    ///
    /// Returns the first [`ProxyRouteError`] of the specs, or the one of
    /// [`from_routes`](Self::from_routes).
    pub fn from_geo_proxies<I, S>(
        items: I,
        rotation: RotationStrategy,
    ) -> Result<Self, ProxyRouteError>
    where
        I: IntoIterator<Item = (S, CountryCode)>,
        S: AsRef<str>,
    {
        let routes = items
            .into_iter()
            .map(|(spec, country)| ProxyRoute::parse(spec.as_ref(), Some(country)))
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_routes(routes, rotation)
    }

    /// Sets how many failures in a row put a proxy out of rotation, and for how long, as core's
    /// [`ProxyList::with_quarantine`] does: a limit of zero is read as one.
    #[must_use]
    pub fn with_quarantine(mut self, after_failures: u32, rest: Duration) -> Self {
        self.list = std::mem::take(&mut self.list).with_quarantine(after_failures, rest);
        self
    }

    /// The proxy for a request to `site` at time `now`, in seconds since the Unix epoch. `None`
    /// when the pool is empty or every proxy is quarantined.
    #[must_use]
    pub fn pick(&self, site: Option<&str>, now: u64) -> Option<ProxyRoute> {
        let entry = self.list.pick_at(site, at(now))?;
        self.route_of(entry).cloned()
    }

    /// Like [`ProxyPool::pick`], limited to proxies in `country`.
    #[must_use]
    pub fn pick_in(
        &self,
        country: CountryCode,
        site: Option<&str>,
        now: u64,
    ) -> Option<ProxyRoute> {
        self.pick_where(|key| key.country == Some(country), site, now)
    }

    /// Like [`ProxyPool::pick`], excluding `current` (and any other route of the same key), so
    /// that a rotation really moves. With [`RotationStrategy::StickyPerSite`] the site then sticks
    /// to the new proxy.
    #[must_use]
    pub fn pick_other(
        &self,
        current: &ProxyKey,
        site: Option<&str>,
        now: u64,
    ) -> Option<ProxyRoute> {
        self.pick_where(|key| key != current, site, now)
    }

    /// Records what happened to a request through `proxy` at time `now`, as core's
    /// [`ProxyList::report_at`] does: a success clears the failure count, any other outcome adds
    /// one, and at the quarantine limit the proxy rests. A proxy the pool does not hold is ignored.
    pub fn report(&self, proxy: &ProxyKey, outcome: ProxyOutcome, now: u64) {
        let entry = self
            .routes
            .iter()
            .position(|route| route.key() == proxy)
            .and_then(|position| self.list.iter().nth(position));
        if let Some(entry) = entry {
            self.list.report_at(entry, outcome, at(now));
        }
    }

    /// The pick of `pick_in` and `pick_other`: among the routes whose key passes `accept`.
    fn pick_where(
        &self,
        accept: impl Fn(&ProxyKey) -> bool,
        site: Option<&str>,
        now: u64,
    ) -> Option<ProxyRoute> {
        let entry = self.list.pick_where_at(
            |entry| {
                self.route_of(entry)
                    .is_some_and(|route| accept(route.key()))
            },
            site,
            at(now),
        )?;
        self.route_of(entry).cloned()
    }

    /// The route of a list entry: the one at the entry's position.
    fn route_of(&self, entry: &ProxyEntry) -> Option<&ProxyRoute> {
        let position = self
            .list
            .iter()
            .position(|held| std::ptr::eq(held, entry))?;
        self.routes.get(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(spec: &str, country: Option<&str>) -> ProxyRoute {
        let country = country.map(|code| CountryCode::new(code).expect("valid code"));
        ProxyRoute::parse(spec, country).expect("valid spec")
    }

    fn key(host: &str) -> ProxyKey {
        route(&format!("http://{host}:8080"), None).key().clone()
    }

    fn three() -> Vec<ProxyRoute> {
        vec![
            route("http://p1.example:8080", None),
            route("http://p2.example:8080", None),
            route("http://p3.example:8080", None),
        ]
    }

    fn pool(routes: Vec<ProxyRoute>, rotation: RotationStrategy) -> ProxyPool {
        ProxyPool::from_routes(routes, rotation).expect("core's list takes these routes")
    }

    fn secs(count: u64) -> Duration {
        Duration::from_secs(count)
    }

    fn host_of(proxy: Option<ProxyRoute>) -> String {
        proxy
            .map(|route| route.key().host.clone())
            .unwrap_or_default()
    }

    /// The hosts of `count` picks in a row at time `now`.
    fn hosts(pool: &ProxyPool, count: usize, now: u64) -> Vec<String> {
        (0..count).map(|_| host_of(pool.pick(None, now))).collect()
    }

    #[test]
    fn the_pool_can_be_shared_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProxyPool>();
    }

    #[test]
    fn round_robin_cycles_through_every_proxy() {
        let pool = pool(three(), RotationStrategy::RoundRobin);
        assert_eq!(
            hosts(&pool, 6, 0),
            ["p1.example", "p2.example", "p3.example"].repeat(2)
        );
    }

    #[test]
    fn an_empty_pool_has_nothing_to_pick() {
        let pool = pool(Vec::new(), RotationStrategy::RoundRobin);
        let de = CountryCode::new("DE").expect("valid");
        assert!(pool.pick(None, 0).is_none());
        assert!(pool.pick_in(de, None, 0).is_none());
        assert!(pool.pick_other(&key("p1.example"), None, 0).is_none());
    }

    #[test]
    fn round_robin_skips_a_quarantined_proxy_without_repeating_another() {
        let pool = pool(three(), RotationStrategy::RoundRobin).with_quarantine(1, secs(600));
        pool.report(&key("p2.example"), ProxyOutcome::ConnectFailed, 0);
        assert_eq!(
            hosts(&pool, 6, 1),
            ["p1.example", "p3.example"].repeat(3),
            "the cycle holds its order around the gap"
        );
    }

    #[test]
    fn a_released_proxy_rejoins_the_cycle() {
        let pool = pool(three(), RotationStrategy::RoundRobin).with_quarantine(1, secs(600));
        pool.report(&key("p2.example"), ProxyOutcome::ConnectFailed, 0);
        let _ = hosts(&pool, 2, 1);
        let mut after = hosts(&pool, 3, 600);
        after.sort();
        assert_eq!(after, ["p1.example", "p2.example", "p3.example"]);
    }

    #[test]
    fn three_failures_quarantine_a_proxy_until_its_time_is_up() {
        let pool = pool(
            vec![
                route("http://p1.example:8080", None),
                route("http://p2.example:8080", None),
            ],
            RotationStrategy::RoundRobin,
        )
        .with_quarantine(3, secs(600));
        for _ in 0..3 {
            pool.report(&key("p1.example"), ProxyOutcome::ConnectFailed, 100);
        }
        assert_eq!(hosts(&pool, 4, 699), ["p2.example"; 4]);
        let released = hosts(&pool, 2, 700);
        assert!(released.contains(&"p1.example".to_owned()), "{released:?}");
    }

    #[test]
    fn the_default_quarantine_is_three_failures_for_ten_minutes() {
        let pool = pool(
            vec![route("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        );
        for _ in 0..DEFAULT_QUARANTINE_AFTER {
            pool.report(&key("p1.example"), ProxyOutcome::Blocked, 10);
        }
        assert!(pool.pick(None, 10 + DEFAULT_QUARANTINE_SECS - 1).is_none());
        assert!(pool.pick(None, 10 + DEFAULT_QUARANTINE_SECS).is_some());
    }

    #[test]
    fn a_released_proxy_starts_again_with_no_failures() {
        let pool = pool(
            vec![route("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        )
        .with_quarantine(2, secs(10));
        for _ in 0..2 {
            pool.report(&key("p1.example"), ProxyOutcome::ConnectFailed, 0);
        }
        assert!(pool.pick(None, 9).is_none());
        pool.report(&key("p1.example"), ProxyOutcome::ConnectFailed, 10);
        assert!(
            pool.pick(None, 11).is_some(),
            "one failure after release is below the limit of two"
        );
    }

    #[test]
    fn a_limit_of_zero_means_the_first_failure() {
        let pool = pool(
            vec![route("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        )
        .with_quarantine(0, secs(60));
        assert!(pool.pick(None, 0).is_some());
        pool.report(&key("p1.example"), ProxyOutcome::ConnectFailed, 0);
        assert!(pool.pick(None, 1).is_none());
    }

    #[test]
    fn every_failing_outcome_counts_and_a_success_clears_the_count() {
        for failing in [
            ProxyOutcome::ConnectFailed,
            ProxyOutcome::Challenged,
            ProxyOutcome::Blocked,
        ] {
            let pool = pool(
                vec![route("http://p1.example:8080", None)],
                RotationStrategy::RoundRobin,
            );
            let p1 = key("p1.example");
            pool.report(&p1, failing, 0);
            pool.report(&p1, failing, 0);
            pool.report(&p1, ProxyOutcome::Success, 0);
            pool.report(&p1, failing, 0);
            pool.report(&p1, failing, 0);
            assert!(
                pool.pick(None, 0).is_some(),
                "two failures after a success must not quarantine ({failing:?})"
            );
            pool.report(&p1, failing, 0);
            assert!(pool.pick(None, 0).is_none(), "the third one does");
        }
    }

    #[test]
    fn a_report_for_a_proxy_the_pool_does_not_hold_changes_nothing() {
        let pool = pool(three(), RotationStrategy::RoundRobin).with_quarantine(1, secs(600));
        pool.report(&key("elsewhere.example"), ProxyOutcome::Blocked, 0);
        assert_eq!(
            hosts(&pool, 3, 0),
            ["p1.example", "p2.example", "p3.example"]
        );
    }

    #[test]
    fn least_failures_avoids_the_proxy_that_failed() {
        let pool = pool(three(), RotationStrategy::LeastFailures);
        pool.report(&key("p1.example"), ProxyOutcome::Challenged, 0);
        for host in hosts(&pool, 4, 0) {
            assert_ne!(host, "p1.example");
        }
    }

    #[test]
    fn least_failures_rotates_among_equals() {
        let pool = pool(three(), RotationStrategy::LeastFailures);
        assert_eq!(
            hosts(&pool, 6, 0),
            ["p1.example", "p2.example", "p3.example"].repeat(2)
        );
    }

    #[test]
    fn least_failures_follows_the_healthiest_proxy() {
        let pool = pool(three(), RotationStrategy::LeastFailures);
        pool.report(&key("p1.example"), ProxyOutcome::ConnectFailed, 0);
        pool.report(&key("p1.example"), ProxyOutcome::ConnectFailed, 0);
        pool.report(&key("p2.example"), ProxyOutcome::ConnectFailed, 0);
        assert_eq!(hosts(&pool, 3, 0), ["p3.example"; 3]);
        pool.report(&key("p3.example"), ProxyOutcome::ConnectFailed, 0);
        assert_eq!(
            hosts(&pool, 4, 0),
            ["p2.example", "p3.example", "p2.example", "p3.example"]
        );
    }

    #[test]
    fn sticky_per_site_keeps_the_first_proxy_for_that_site() {
        let pool = pool(three(), RotationStrategy::StickyPerSite);
        let first = host_of(pool.pick(Some("a.example"), 0));
        for _ in 0..5 {
            assert_eq!(host_of(pool.pick(Some("a.example"), 0)), first);
        }
    }

    #[test]
    fn sticky_sites_are_independent_and_new_ones_move_on() {
        let pool = pool(three(), RotationStrategy::StickyPerSite);
        let a = host_of(pool.pick(Some("a.example"), 0));
        let b = host_of(pool.pick(Some("b.example"), 0));
        let c = host_of(pool.pick(Some("c.example"), 0));
        assert_eq!(
            [a.as_str(), b.as_str(), c.as_str()],
            ["p1.example", "p2.example", "p3.example"]
        );
        assert_eq!(host_of(pool.pick(Some("a.example"), 0)), a);
        assert_eq!(host_of(pool.pick(Some("b.example"), 0)), b);
    }

    #[test]
    fn sticky_without_a_site_rotates_like_round_robin() {
        let pool = pool(three(), RotationStrategy::StickyPerSite);
        assert_eq!(
            hosts(&pool, 3, 0),
            ["p1.example", "p2.example", "p3.example"]
        );
    }

    #[test]
    fn a_sticky_proxy_that_is_quarantined_gives_way() {
        let pool = pool(three(), RotationStrategy::StickyPerSite).with_quarantine(1, secs(600));
        let first = pool.pick(Some("a.example"), 0).expect("a proxy");
        pool.report(first.key(), ProxyOutcome::Blocked, 0);
        let next = host_of(pool.pick(Some("a.example"), 0));
        assert_ne!(next, first.key().host);
        assert_eq!(
            host_of(pool.pick(Some("a.example"), 0)),
            next,
            "the site now sticks to the replacement"
        );
    }

    #[test]
    fn random_with_one_seed_gives_one_order() {
        let order = |seed| {
            let pool = pool(three(), RotationStrategy::Random { seed: Some(seed) });
            hosts(&pool, 10, 0)
        };
        assert_eq!(order(11), order(11));
    }

    #[test]
    fn random_uses_every_proxy_and_never_a_quarantined_one() {
        for seed in [Some(3), None] {
            let open = pool(three(), RotationStrategy::Random { seed });
            let seen = hosts(&open, 60, 0);
            for host in ["p1.example", "p2.example", "p3.example"] {
                assert!(seen.iter().any(|h| h == host), "{host} never drawn");
            }
            let resting =
                pool(three(), RotationStrategy::Random { seed }).with_quarantine(1, secs(600));
            resting.report(&key("p2.example"), ProxyOutcome::Blocked, 0);
            assert!(!hosts(&resting, 60, 1).contains(&"p2.example".to_owned()));
        }
    }

    #[test]
    fn pick_other_never_returns_the_current_proxy() {
        let pool = pool(three(), RotationStrategy::RoundRobin);
        let current = key("p2.example");
        for _ in 0..6 {
            assert_ne!(host_of(pool.pick_other(&current, None, 0)), "p2.example");
        }
    }

    #[test]
    fn pick_other_with_nothing_else_left_is_none() {
        let pool = pool(
            vec![route("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        );
        assert!(pool.pick_other(&key("p1.example"), None, 0).is_none());
    }

    #[test]
    fn pick_other_refuses_every_route_of_the_current_key() {
        let pool = pool(
            vec![
                route("http://p1.example:8080", None),
                route("http://p1.example:8080", None),
                route("http://p2.example:8080", None),
            ],
            RotationStrategy::RoundRobin,
        );
        for _ in 0..6 {
            assert_eq!(
                host_of(pool.pick_other(&key("p1.example"), None, 0)),
                "p2.example"
            );
        }
    }

    #[test]
    fn pick_other_moves_a_sticky_site_to_the_new_proxy() {
        let pool = pool(three(), RotationStrategy::StickyPerSite);
        let first = pool.pick(Some("a.example"), 0).expect("a proxy");
        let moved = pool
            .pick_other(first.key(), Some("a.example"), 0)
            .expect("another");
        assert_ne!(moved.key(), first.key());
        assert_eq!(
            pool.pick(Some("a.example"), 0).expect("sticky").key(),
            moved.key()
        );
    }

    #[test]
    fn pick_in_only_returns_proxies_in_that_country() {
        let de = CountryCode::new("DE").expect("valid");
        let pool = pool(
            vec![
                route("http://fr.example:8080", Some("FR")),
                route("http://de.example:8080", Some("DE")),
            ],
            RotationStrategy::RoundRobin,
        );
        for _ in 0..4 {
            assert_eq!(host_of(pool.pick_in(de, None, 0)), "de.example");
        }
        let jp = CountryCode::new("JP").expect("valid");
        assert!(pool.pick_in(jp, None, 0).is_none());
    }

    #[test]
    fn pick_in_finds_nothing_when_the_whole_country_is_quarantined() {
        let de = CountryCode::new("DE").expect("valid");
        let pool = pool(
            vec![
                route("http://fr.example:8080", Some("FR")),
                route("http://de.example:8080", Some("DE")),
            ],
            RotationStrategy::RoundRobin,
        )
        .with_quarantine(1, secs(600));
        pool.report(
            &key_with_country("de.example", "DE"),
            ProxyOutcome::Blocked,
            0,
        );
        assert!(pool.pick_in(de, None, 1).is_none());
        assert!(
            pool.pick(None, 1).is_some(),
            "the other country still serves"
        );
    }

    fn key_with_country(host: &str, country: &str) -> ProxyKey {
        route(&format!("http://{host}:8080"), Some(country))
            .key()
            .clone()
    }

    #[test]
    fn from_geo_proxies_sets_the_country_on_every_key() {
        let de = CountryCode::new("DE").expect("valid");
        let fr = CountryCode::new("FR").expect("valid");
        let pool = ProxyPool::from_geo_proxies(
            [("de.example:8080", de), ("http://fr.example:3128", fr)],
            RotationStrategy::RoundRobin,
        )
        .expect("valid specs");
        assert_eq!(pool.pick_in(fr, None, 0).expect("French").key().port, 3128);
        assert_eq!(
            pool.pick_in(de, None, 0).expect("German").key().country,
            Some(de)
        );
    }

    #[test]
    fn from_geo_proxies_refuses_a_bad_spec() {
        let de = CountryCode::new("DE").expect("valid");
        let result =
            ProxyPool::from_geo_proxies([("ftp://x.example:21", de)], RotationStrategy::RoundRobin);
        assert!(result.is_err());
    }

    #[test]
    fn a_core_list_becomes_a_pool_that_keeps_its_rotation_and_quarantine() {
        let list = ProxyList::parse("p1.example:8080\np2.example:8080\np3.example:8080\n")
            .expect("a list")
            .with_rotation(RotationStrategy::StickyPerSite)
            .with_quarantine(1, secs(600));
        let pool = ProxyPool::from_list(list).expect("routes");
        let first = pool.pick(Some("a.example"), 0).expect("a proxy");
        let other = pool.pick(Some("b.example"), 0).expect("a proxy");
        assert_ne!(first.key(), other.key(), "a new site moves on");
        assert_eq!(
            pool.pick(Some("a.example"), 0).expect("sticky").key(),
            first.key()
        );
        pool.report(first.key(), ProxyOutcome::Blocked, 0);
        assert_ne!(
            pool.pick(Some("a.example"), 1).expect("another").key(),
            first.key(),
            "the quarantine of the list holds"
        );
        assert!(first.key().country.is_none(), "a list entry has no country");
    }

    #[test]
    fn a_core_list_entry_that_is_no_route_is_refused() {
        let list = ProxyList::parse("socks4://s.example:1080\n").expect("core takes socks4");
        let error = ProxyPool::from_list(list).expect_err("a route has no socks4");
        assert!(matches!(error, ProxyRouteError::Scheme(scheme) if scheme == "socks4"));
    }

    #[test]
    fn a_route_cores_list_cannot_hold_is_an_error_not_a_panic() {
        let odd = route("socks5://%zz.example:1080", None);
        let error = ProxyPool::from_routes(vec![odd], RotationStrategy::RoundRobin)
            .expect_err("core's list refuses the host");
        assert!(matches!(error, ProxyRouteError::Address(_)), "{error}");
    }

    #[test]
    fn a_socks5h_route_is_held_and_returned_as_it_is() {
        let pool = pool(
            vec![route("socks5h://10.0.0.1:1080", None)],
            RotationStrategy::RoundRobin,
        );
        let picked = pool.pick(None, 0).expect("a proxy");
        assert_eq!(picked.key().scheme, "socks5h");
        pool.report(picked.key(), ProxyOutcome::Blocked, 0);
        assert_eq!(pool.pick(None, 0).expect("still there").key(), picked.key());
    }

    #[test]
    fn a_time_past_the_end_of_the_calendar_is_read_as_its_last_second() {
        let pool = pool(
            vec![route("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        )
        .with_quarantine(1, secs(60));
        pool.report(&key("p1.example"), ProxyOutcome::Blocked, u64::MAX);
        assert!(pool.pick(None, u64::MAX).is_none());
    }

    #[test]
    fn the_pool_and_its_picks_never_print_a_credential() {
        let pool = ProxyPool::from_geo_proxies(
            [(
                "http://alice:s3cret@p1.example:8080",
                CountryCode::new("DE").expect("valid"),
            )],
            RotationStrategy::StickyPerSite,
        )
        .expect("valid spec");
        let picked = pool.pick(Some("a.example"), 0).expect("a proxy");
        let shown = format!("{pool:?} {picked:?} {picked} {:?}", picked.key());
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
        assert!(
            picked.proxy().has_credentials(),
            "the credentials are still there to use"
        );
    }

    #[test]
    fn a_core_list_with_credentials_keeps_them_out_of_every_view() {
        let list = ProxyList::parse("alice:s3cret@p1.example:8080\n").expect("a list");
        let pool = ProxyPool::from_list(list).expect("routes");
        let picked = pool.pick(None, 0).expect("a proxy");
        let shown = format!("{pool:?} {picked:?} {picked}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
        assert!(picked.proxy().has_credentials());
    }
}
