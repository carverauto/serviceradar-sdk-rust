use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use url::Url;

use crate::http::parse_http_date;

/// One RFC 6265 cookie stored in an ephemeral [`CookieJar`].
///
/// This type intentionally does not implement `Serialize` or `Deserialize`, and
/// its `Debug` output redacts the cookie value.
#[derive(Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    value: String,
    pub domain: String,
    pub host_only: bool,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub expires_at: Option<SystemTime>,
    pub persistent: bool,
    created_seq: u64,
}

impl Cookie {
    /// Returns the cookie value in memory. Never serialize or log this value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cookie")
            .field("name", &self.name)
            .field("domain", &self.domain)
            .field("host_only", &self.host_only)
            .field("path", &self.path)
            .field("secure", &self.secure)
            .field("http_only", &self.http_only)
            .field("value", &"[REDACTED]")
            .finish()
    }
}

/// Ephemeral, in-memory RFC 6265 cookie jar scoped to a single journey run.
///
/// Intentionally non-serializable and redacting in `Debug` output so session
/// cookies never leak into results, telemetry, or logs.
#[derive(Clone, Default)]
pub struct CookieJar {
    cookies: Vec<Cookie>,
    seq: u64,
}

impl fmt::Debug for CookieJar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieJar")
            .field("count", &self.cookies.len())
            .field("cookies", &"[REDACTED]")
            .finish()
    }
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store_set_cookies(&mut self, request_url: &str, set_cookies: &[String]) {
        self.store_set_cookies_at(request_url, set_cookies, SystemTime::now());
    }

    pub fn store_set_cookies_at(
        &mut self,
        request_url: &str,
        set_cookies: &[String],
        now: SystemTime,
    ) {
        if set_cookies.is_empty() {
            return;
        }
        let Ok(parsed) = Url::parse(request_url.trim()) else {
            return;
        };
        let Some(host) = parsed.host_str().map(normalize_cookie_host) else {
            return;
        };
        if host.is_empty() {
            return;
        }
        let req_path = if parsed.path().is_empty() {
            "/"
        } else {
            parsed.path()
        };

        for header in set_cookies {
            self.store_one_at(&host, req_path, header, now);
        }
    }

    fn store_one_at(&mut self, req_host: &str, req_path: &str, header: &str, now: SystemTime) {
        let mut parts = header.split(';');
        let Some(name_val) = parts.next() else {
            return;
        };
        let Some((raw_name, raw_val)) = name_val.split_once('=') else {
            return;
        };
        let name = raw_name.trim();
        let value = raw_val.trim();
        if name.is_empty() {
            return;
        }

        let mut domain = req_host.to_string();
        let mut host_only = true;
        let mut path = default_cookie_path(req_path);
        let mut secure = false;
        let mut http_only = false;
        let mut max_age: Option<i64> = None;
        let mut expires: Option<SystemTime> = None;

        for raw_attr in parts {
            let attr = raw_attr.trim();
            if attr.is_empty() {
                continue;
            }
            let (attr_name, attr_val) = match attr.split_once('=') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => (attr, ""),
            };
            if attr_name.eq_ignore_ascii_case("domain") {
                let candidate = normalize_cookie_host(attr_val.trim_start_matches('.'));
                if candidate.is_empty() {
                    continue;
                }
                if !domain_matches(req_host, &candidate)
                    || (req_host != candidate && !candidate.contains('.'))
                {
                    return;
                }
                domain = candidate;
                host_only = false;
            } else if attr_name.eq_ignore_ascii_case("path") {
                if attr_val.starts_with('/') {
                    path = attr_val.to_string();
                } else {
                    path = default_cookie_path(req_path);
                }
            } else if attr_name.eq_ignore_ascii_case("secure") {
                secure = true;
            } else if attr_name.eq_ignore_ascii_case("httponly") {
                http_only = true;
            } else if attr_name.eq_ignore_ascii_case("max-age") {
                if let Ok(secs) = attr_val.parse::<i64>() {
                    max_age = Some(secs);
                }
            } else if attr_name.eq_ignore_ascii_case("expires")
                && let Some(parsed) = parse_http_date(attr_val)
            {
                expires = Some(parsed);
            }
        }

        let (expires_at, persistent) = if let Some(secs) = max_age {
            if secs <= 0 {
                (Some(UNIX_EPOCH), true)
            } else {
                (now.checked_add(Duration::from_secs(secs as u64)), true)
            }
        } else if let Some(at) = expires {
            (Some(at), true)
        } else {
            (None, false)
        };

        if let Some(at) = expires_at
            && at <= now
        {
            self.delete_cookie(name, &domain, &path);
            return;
        }

        let cookie = Cookie {
            name: name.to_string(),
            value: value.to_string(),
            domain,
            host_only,
            path,
            secure,
            http_only,
            expires_at,
            persistent,
            created_seq: 0,
        };
        self.upsert_cookie(cookie);
    }

    fn delete_cookie(&mut self, name: &str, domain: &str, path: &str) {
        self.cookies
            .retain(|c| !(c.name == name && c.domain == domain && c.path == path));
    }

    fn upsert_cookie(&mut self, mut cookie: Cookie) {
        if let Some(existing) = self
            .cookies
            .iter_mut()
            .find(|c| c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path)
        {
            cookie.created_seq = existing.created_seq;
            *existing = cookie;
            return;
        }
        self.seq += 1;
        cookie.created_seq = self.seq;
        self.cookies.push(cookie);
    }

    pub fn cookies_for_url(&mut self, request_url: &str) -> Vec<&Cookie> {
        self.cookies_for_url_at(request_url, SystemTime::now())
    }

    pub fn cookies_for_url_at(&mut self, request_url: &str, now: SystemTime) -> Vec<&Cookie> {
        self.cookies
            .retain(|c| c.expires_at.is_none_or(|at| at > now));

        let Ok(parsed) = Url::parse(request_url.trim()) else {
            return Vec::new();
        };
        let scheme = parsed.scheme().to_ascii_lowercase();
        let Some(req_host) = parsed.host_str().map(normalize_cookie_host) else {
            return Vec::new();
        };
        if req_host.is_empty() {
            return Vec::new();
        }
        let req_path = if parsed.path().is_empty() {
            "/"
        } else {
            parsed.path()
        };

        let mut matches: Vec<&Cookie> = self
            .cookies
            .iter()
            .filter(|c| {
                if c.secure && scheme != "https" {
                    return false;
                }
                let dom_ok = if c.host_only {
                    req_host == c.domain
                } else {
                    domain_matches(&req_host, &c.domain)
                };
                dom_ok && path_matches(req_path, &c.path)
            })
            .collect();

        matches.sort_by(|a, b| {
            b.path
                .len()
                .cmp(&a.path.len())
                .then_with(|| a.created_seq.cmp(&b.created_seq))
        });
        matches
    }

    pub fn cookie_header_for_url(&mut self, request_url: &str) -> Option<String> {
        self.cookie_header_for_url_at(request_url, SystemTime::now())
    }

    pub fn cookie_header_for_url_at(
        &mut self,
        request_url: &str,
        now: SystemTime,
    ) -> Option<String> {
        let matches = self.cookies_for_url_at(request_url, now);
        if matches.is_empty() {
            return None;
        }
        let mut out = String::new();
        for (idx, c) in matches.into_iter().enumerate() {
            if idx > 0 {
                out.push_str("; ");
            }
            out.push_str(&c.name);
            out.push('=');
            out.push_str(&c.value);
        }
        Some(out)
    }

    pub fn cookie_value(&mut self, request_url: &str, name: &str) -> Option<String> {
        self.cookie_value_at(request_url, name, SystemTime::now())
    }

    pub fn cookie_value_at(
        &mut self,
        request_url: &str,
        name: &str,
        now: SystemTime,
    ) -> Option<String> {
        self.cookies_for_url_at(request_url, now)
            .into_iter()
            .find(|c| c.name == name)
            .map(|c| c.value.clone())
    }

    pub(crate) fn latest_value_by_name(&self, name: &str) -> Option<String> {
        self.cookies
            .iter()
            .rev()
            .find(|c| c.name == name)
            .map(|c| c.value.clone())
    }
}

fn normalize_cookie_host(host: &str) -> String {
    let trimmed = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.');
    trimmed.to_ascii_lowercase()
}

fn default_cookie_path(req_path: &str) -> String {
    if req_path.is_empty() || !req_path.starts_with('/') || req_path == "/" {
        return "/".to_string();
    }
    match req_path.rfind('/') {
        Some(idx) if idx > 0 => req_path[..idx].to_string(),
        _ => "/".to_string(),
    }
}

fn domain_matches(req_host: &str, domain: &str) -> bool {
    if req_host == domain {
        return true;
    }
    if is_ip_literal(req_host) || is_ip_literal(domain) {
        return false;
    }
    req_host.len() > domain.len()
        && req_host.ends_with(domain)
        && req_host.as_bytes()[req_host.len() - domain.len() - 1] == b'.'
}

fn path_matches(req_path: &str, cookie_path: &str) -> bool {
    if req_path == cookie_path {
        return true;
    }
    if !req_path.starts_with(cookie_path) {
        return false;
    }
    if cookie_path.ends_with('/') {
        return true;
    }
    req_path.as_bytes().get(cookie_path.len()) == Some(&b'/')
}

fn is_ip_literal(host: &str) -> bool {
    if host.contains(':') {
        return true;
    }
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts
        .into_iter()
        .all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u8>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_jar_redacts_debug_output() {
        let mut jar = CookieJar::new();
        jar.store_set_cookies(
            "https://sso.example.com/login",
            &["sid=super-secret-cookie; Path=/; Secure".to_string()],
        );
        let debug = format!("{jar:?}");
        assert!(!debug.contains("super-secret-cookie"));
        let matches = jar.cookies_for_url("https://sso.example.com/app");
        assert_eq!(matches.len(), 1);
        let cookie_debug = format!("{:?}", matches[0]);
        assert!(!cookie_debug.contains("super-secret-cookie"));
    }
}
