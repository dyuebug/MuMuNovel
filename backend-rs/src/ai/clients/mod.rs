pub mod anthropic;
pub mod gemini;
pub mod openai;

pub(super) fn should_bypass_system_proxy(base_url: &str) -> bool {
    reqwest::Url::parse(base_url.trim())
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .map(|host| {
            matches!(
                host.as_str(),
                "127.0.0.1" | "localhost" | "::1" | "[::1]" | "host.docker.internal"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::should_bypass_system_proxy;

    #[test]
    fn system_proxy_bypass_is_limited_to_local_gateway_hosts() {
        for base_url in [
            "http://127.0.0.1:8317/v1",
            "http://localhost:8317/v1",
            "http://[::1]:8317/v1",
            "http://host.docker.internal:8317/v1",
        ] {
            assert!(should_bypass_system_proxy(base_url), "{base_url}");
        }

        for base_url in [
            "https://api.openai.com/v1",
            "https://generativelanguage.googleapis.com/v1beta",
            "not-a-url",
        ] {
            assert!(!should_bypass_system_proxy(base_url), "{base_url}");
        }
    }
}
