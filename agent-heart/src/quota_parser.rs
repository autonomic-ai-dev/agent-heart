//! Rate limit and quota response header parser for LLM providers.
//! Supports Anthropic, OpenAI, and Google Gemini header conventions.

use crate::usage_schema::ProviderQuotaSnapshot;
use std::collections::HashMap;

/// Parse HTTP response headers into structured provider quota snapshots.
/// Headers map is treated case-insensitively.
pub fn parse_provider_headers(
    provider_hint: Option<&str>,
    headers: &HashMap<String, String>,
    account_or_tier: Option<&str>,
) -> Vec<ProviderQuotaSnapshot> {
    // Normalize header keys to lowercase
    let lower_headers: HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
        .collect();

    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut snapshots = Vec::new();

    let provider = provider_hint
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| detect_provider(&lower_headers));

    match provider.as_str() {
        "anthropic" => {
            if let Some(snap) = parse_anthropic_headers(&lower_headers, account_or_tier, now_ms) {
                snapshots.push(snap);
            }
        }
        "openai" => {
            snapshots.extend(parse_openai_headers(
                &lower_headers,
                account_or_tier,
                now_ms,
            ));
        }
        "gemini" | "google" => {
            snapshots.extend(parse_gemini_headers(
                &lower_headers,
                account_or_tier,
                now_ms,
            ));
        }
        _ => {
            // Check all known formats
            if let Some(snap) = parse_anthropic_headers(&lower_headers, account_or_tier, now_ms) {
                snapshots.push(snap);
            }
            snapshots.extend(parse_openai_headers(
                &lower_headers,
                account_or_tier,
                now_ms,
            ));
            snapshots.extend(parse_gemini_headers(
                &lower_headers,
                account_or_tier,
                now_ms,
            ));
        }
    }

    snapshots
}

fn detect_provider(headers: &HashMap<String, String>) -> String {
    for key in headers.keys() {
        if key.starts_with("anthropic-ratelimit-") {
            return "anthropic".to_string();
        }
        if key.starts_with("x-ratelimit-") {
            return "openai".to_string();
        }
        if key.starts_with("x-goog-ratelimit-") {
            return "gemini".to_string();
        }
    }
    "unknown".to_string()
}

fn parse_duration_to_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Ok(secs) = s.parse::<f64>() {
        return Some((secs * 1000.0) as i64);
    }
    // Formats like "6m0s", "15ms", "2s"
    if s.ends_with("ms") {
        return s.trim_end_matches("ms").parse::<i64>().ok();
    }
    if s.ends_with('s') && !s.contains('m') {
        return s
            .trim_end_matches('s')
            .parse::<f64>()
            .ok()
            .map(|sec| (sec * 1000.0) as i64);
    }
    if s.contains('m') && s.ends_with('s') {
        let parts: Vec<&str> = s.split('m').collect();
        if parts.len() == 2 {
            let mins: f64 = parts[0].parse().ok()?;
            let secs: f64 = parts[1].trim_end_matches('s').parse().ok()?;
            return Some(((mins * 60.0 + secs) * 1000.0) as i64);
        }
    }
    None
}

fn parse_reset_timestamp(value: &str, now_ms: i64) -> Option<i64> {
    let value = value.trim();
    // Check if integer (epoch ms or secs)
    if let Ok(num) = value.parse::<i64>() {
        if num > 1_000_000_000_000 {
            // Already ms epoch
            return Some(num);
        } else if num > 1_000_000_000 {
            // Seconds epoch
            return Some(num * 1000);
        }
    }
    // Check if relative duration
    if let Some(ms_offset) = parse_duration_to_ms(value) {
        return Some(now_ms + ms_offset);
    }
    // Check if ISO8601 string
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(dt.timestamp_millis());
    }
    None
}

fn parse_anthropic_headers(
    headers: &HashMap<String, String>,
    account_or_tier: Option<&str>,
    now_ms: i64,
) -> Option<ProviderQuotaSnapshot> {
    let mut utilization_pct = None;

    if let Some(u_str) = headers.get("anthropic-ratelimit-unified-5h-utilization") {
        if let Ok(val) = u_str.parse::<f64>() {
            let pct = if val <= 1.0 { val * 100.0 } else { val };
            utilization_pct = Some(pct.clamp(0.0, 100.0));
        }
    }

    let remaining_tokens = headers
        .get("anthropic-ratelimit-tokens-remaining")
        .and_then(|s| s.parse::<u64>().ok());
    let limit_tokens = headers
        .get("anthropic-ratelimit-tokens-limit")
        .and_then(|s| s.parse::<u64>().ok());

    if utilization_pct.is_none() {
        if let (Some(rem), Some(lim)) = (remaining_tokens, limit_tokens) {
            if lim > 0 {
                let pct = (1.0 - (rem as f64 / lim as f64)) * 100.0;
                utilization_pct = Some(pct.clamp(0.0, 100.0));
            }
        }
    }

    let reset_at = headers
        .get("anthropic-ratelimit-unified-5h-reset")
        .or_else(|| headers.get("anthropic-ratelimit-tokens-reset"))
        .and_then(|s| parse_reset_timestamp(s, now_ms));

    if utilization_pct.is_none() && remaining_tokens.is_none() && reset_at.is_none() {
        return None;
    }

    Some(ProviderQuotaSnapshot {
        provider: "anthropic".to_string(),
        account_or_tier: account_or_tier
            .unwrap_or("Claude Code (5h rolling)")
            .to_string(),
        window_type: "5h_rolling".to_string(),
        utilization_pct: utilization_pct.unwrap_or(0.0),
        reset_at,
        rate_limit_remaining: remaining_tokens,
        updated_at: now_ms,
    })
}

fn parse_openai_headers(
    headers: &HashMap<String, String>,
    account_or_tier: Option<&str>,
    now_ms: i64,
) -> Vec<ProviderQuotaSnapshot> {
    let mut snapshots = Vec::new();

    // 1. Tokens per minute (TPM)
    let rem_tokens = headers
        .get("x-ratelimit-remaining-tokens")
        .and_then(|s| s.parse::<u64>().ok());
    let lim_tokens = headers
        .get("x-ratelimit-limit-tokens")
        .and_then(|s| s.parse::<u64>().ok());
    let reset_tokens = headers
        .get("x-ratelimit-reset-tokens")
        .and_then(|s| parse_reset_timestamp(s, now_ms));

    if rem_tokens.is_some() || lim_tokens.is_some() {
        let util_pct = match (rem_tokens, lim_tokens) {
            (Some(rem), Some(lim)) if lim > 0 => {
                ((1.0 - (rem as f64 / lim as f64)) * 100.0).clamp(0.0, 100.0)
            }
            _ => 0.0,
        };

        snapshots.push(ProviderQuotaSnapshot {
            provider: "openai".to_string(),
            account_or_tier: account_or_tier.unwrap_or("Tier 4 (TPM)").to_string(),
            window_type: "tpm".to_string(),
            utilization_pct: util_pct,
            reset_at: reset_tokens,
            rate_limit_remaining: rem_tokens,
            updated_at: now_ms,
        });
    }

    // 2. Requests per minute (RPM)
    let rem_reqs = headers
        .get("x-ratelimit-remaining-requests")
        .and_then(|s| s.parse::<u64>().ok());
    let lim_reqs = headers
        .get("x-ratelimit-limit-requests")
        .and_then(|s| s.parse::<u64>().ok());
    let reset_reqs = headers
        .get("x-ratelimit-reset-requests")
        .and_then(|s| parse_reset_timestamp(s, now_ms));

    if rem_reqs.is_some() || lim_reqs.is_some() {
        let util_pct = match (rem_reqs, lim_reqs) {
            (Some(rem), Some(lim)) if lim > 0 => {
                ((1.0 - (rem as f64 / lim as f64)) * 100.0).clamp(0.0, 100.0)
            }
            _ => 0.0,
        };

        snapshots.push(ProviderQuotaSnapshot {
            provider: "openai".to_string(),
            account_or_tier: account_or_tier.unwrap_or("Tier 4 (RPM)").to_string(),
            window_type: "rpm".to_string(),
            utilization_pct: util_pct,
            reset_at: reset_reqs,
            rate_limit_remaining: rem_reqs,
            updated_at: now_ms,
        });
    }

    snapshots
}

fn parse_gemini_headers(
    headers: &HashMap<String, String>,
    account_or_tier: Option<&str>,
    now_ms: i64,
) -> Vec<ProviderQuotaSnapshot> {
    let mut snapshots = Vec::new();

    let rem_reqs = headers
        .get("x-goog-ratelimit-remaining-requests")
        .and_then(|s| s.parse::<u64>().ok());
    let lim_reqs = headers
        .get("x-goog-ratelimit-limit-requests")
        .and_then(|s| s.parse::<u64>().ok());
    let reset = headers
        .get("x-goog-ratelimit-reset")
        .and_then(|s| parse_reset_timestamp(s, now_ms));

    if rem_reqs.is_some() || lim_reqs.is_some() {
        let util_pct = match (rem_reqs, lim_reqs) {
            (Some(rem), Some(lim)) if lim > 0 => {
                ((1.0 - (rem as f64 / lim as f64)) * 100.0).clamp(0.0, 100.0)
            }
            _ => 0.0,
        };

        snapshots.push(ProviderQuotaSnapshot {
            provider: "gemini".to_string(),
            account_or_tier: account_or_tier.unwrap_or("Pay-as-you-go").to_string(),
            window_type: "rpm".to_string(),
            utilization_pct: util_pct,
            reset_at: reset,
            rate_limit_remaining: rem_reqs,
            updated_at: now_ms,
        });
    }

    snapshots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_anthropic_headers_direct_utilization() {
        let mut headers = HashMap::new();
        headers.insert(
            "anthropic-ratelimit-unified-5h-utilization".to_string(),
            "0.58".to_string(),
        );
        headers.insert(
            "anthropic-ratelimit-unified-5h-reset".to_string(),
            "1776451200000".to_string(),
        );
        headers.insert(
            "anthropic-ratelimit-tokens-remaining".to_string(),
            "42000".to_string(),
        );

        let snaps = parse_provider_headers(Some("anthropic"), &headers, None);
        assert_eq!(snaps.len(), 1);
        let snap = &snaps[0];
        assert_eq!(snap.provider, "anthropic");
        assert_eq!(snap.window_type, "5h_rolling");
        assert!((snap.utilization_pct - 58.0).abs() < 0.01);
        assert_eq!(snap.reset_at, Some(1776451200000));
        assert_eq!(snap.rate_limit_remaining, Some(42000));
    }

    #[test]
    fn test_parse_openai_headers_token_limit() {
        let mut headers = HashMap::new();
        headers.insert(
            "X-RateLimit-Limit-Tokens".to_string(),
            "1000000".to_string(),
        );
        headers.insert(
            "X-RateLimit-Remaining-Tokens".to_string(),
            "250000".to_string(),
        );
        headers.insert("X-RateLimit-Reset-Tokens".to_string(), "15s".to_string());

        let snaps = parse_provider_headers(None, &headers, Some("Tier 4 (TPM)"));
        assert_eq!(snaps.len(), 1);
        let snap = &snaps[0];
        assert_eq!(snap.provider, "openai");
        assert_eq!(snap.window_type, "tpm");
        assert_eq!(snap.account_or_tier, "Tier 4 (TPM)");
        assert!((snap.utilization_pct - 75.0).abs() < 0.01);
        assert_eq!(snap.rate_limit_remaining, Some(250000));
        assert!(snap.reset_at.is_some());
    }

    #[test]
    fn test_parse_gemini_headers() {
        let mut headers = HashMap::new();
        headers.insert(
            "x-goog-ratelimit-limit-requests".to_string(),
            "1000".to_string(),
        );
        headers.insert(
            "x-goog-ratelimit-remaining-requests".to_string(),
            "800".to_string(),
        );

        let snaps = parse_provider_headers(Some("gemini"), &headers, None);
        assert_eq!(snaps.len(), 1);
        let snap = &snaps[0];
        assert_eq!(snap.provider, "gemini");
        assert_eq!(snap.window_type, "rpm");
        assert!((snap.utilization_pct - 20.0).abs() < 0.01);
        assert_eq!(snap.rate_limit_remaining, Some(800));
    }
}
