//! Microsoft Entra sign-in for SQL Server and Azure SQL: a token from the Azure CLI (`az login`).
//! No password is stored. The token is kept in memory until 5 minutes before it expires.

use serde::Deserialize;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const RESOURCE: &str = "https://database.windows.net/";

static CACHE: Mutex<Option<(String, u64)>> = Mutex::new(None);

#[derive(Deserialize)]
struct AzToken {
    #[serde(rename = "accessToken")]
    access_token: String,
    /// Seconds since 1970 (newer az). Older versions only give `expiresOn` as local text.
    expires_on: Option<u64>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Reads `az account get-access-token` output. Without an expiry, the token is kept for 30 minutes.
pub fn parse(json: &str) -> Result<(String, u64), String> {
    let t: AzToken = serde_json::from_str(json).map_err(|e| format!("Unexpected answer from az: {e}"))?;
    if t.access_token.is_empty() {
        return Err("az gave an empty token.".into());
    }
    Ok((t.access_token, t.expires_on.unwrap_or(now() + 30 * 60)))
}

/// Turns az's error text into a plain message.
pub fn explain(stderr: &str) -> String {
    let e = stderr.trim();
    if e.contains("az login") || e.contains("AADSTS50173") || e.contains("AADSTS700082") || e.contains("expired") {
        return "Your Azure sign-in has ended. Run az login in a terminal, then try again.".into();
    }
    if e.contains("AADSTS53003") || e.to_lowercase().contains("conditional access") {
        return format!("Your company's sign-in rules blocked the token (Conditional Access).\n{e}");
    }
    format!("az could not get a token for SQL Server.\n{e}")
}

fn fetch() -> Result<(String, u64), String> {
    let az = if cfg!(windows) { "az.cmd" } else { "az" };
    let mut c = Command::new(az);
    c.args(["account", "get-access-token", "--resource", RESOURCE, "--output", "json"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // no console window
    }
    let out = c.output().map_err(|_| "Microsoft Entra sign-in uses the Azure CLI, and Tern could not find az. Install it, run az login, and try again.".to_string())?;
    if !out.status.success() {
        return Err(explain(&String::from_utf8_lossy(&out.stderr)));
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

/// A token for SQL Server, from memory while it has more than 5 minutes left, else fresh from az.
pub async fn token() -> Result<String, String> {
    if let Some((t, exp)) = CACHE.lock().unwrap().clone() {
        if exp > now() + 300 {
            return Ok(t);
        }
    }
    let (t, exp) = tauri::async_runtime::spawn_blocking(fetch).await.map_err(|e| e.to_string())??;
    *CACHE.lock().unwrap() = Some((t.clone(), exp));
    Ok(t)
}

/// Forgets the token, so the next sign-in asks az again (after a refused login).
pub fn forget() {
    *CACHE.lock().unwrap() = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_az_output() {
        let (t, exp) = parse(r#"{"accessToken":"abc","expiresOn":"2026-10-18 13:03:43.000000","expires_on":1792346623,"tenant":"x","tokenType":"Bearer"}"#).unwrap();
        assert_eq!((t.as_str(), exp), ("abc", 1792346623));
        let (_, exp) = parse(r#"{"accessToken":"abc","expiresOn":"2026-10-18 13:03:43.000000"}"#).unwrap();
        assert!(exp > now());
        assert!(parse(r#"{"accessToken":""}"#).is_err());
        assert!(parse("not json").is_err());
    }

    #[test]
    fn explains_az_errors() {
        assert!(explain("ERROR: Please run 'az login' to setup account.").contains("Run az login"));
        assert!(explain("AADSTS53003: Access has been blocked by Conditional Access policies.").contains("Conditional Access"));
        assert!(explain("something else").starts_with("az could not get a token"));
    }
}
