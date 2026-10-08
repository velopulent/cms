use crate::config::Config;
use crate::grpc::interceptor::compute_key_hmac;

/// Lightweight auth context extracted from the Bearer token.
/// This is inserted into request extensions by the interceptor.
#[derive(Clone, Debug)]
pub struct AuthContext {
    pub hmac: String,
    pub personal: bool,
}

/// Returned when a raw Bearer token fails format validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidToken;

/// Parse and validate the raw Bearer token format.
///
/// Returns an `AuthContext` on success, or `InvalidToken` on any validation failure.
pub fn parse_token(token: &str, config: &Config) -> Result<AuthContext, InvalidToken> {
    let personal = token.starts_with("vcms_pat_");
    if !personal && !token.starts_with("vcms_site_") {
        return Err(InvalidToken);
    }

    let prefix_len = if personal {
        "vcms_pat_".len()
    } else {
        "vcms_site_".len()
    };
    if token.len() <= prefix_len {
        return Err(InvalidToken);
    }
    let hmac = compute_key_hmac(token, &config.token_index_key);

    Ok(AuthContext { hmac, personal })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_token_valid() {
        let config = Config {
            token_index_key: "secret".to_string(),
            ..Default::default()
        };
        let token = "vcms_site_abc1234567890123456";
        let ctx = parse_token(token, &config).unwrap();
        assert_eq!(ctx.hmac.len(), 64);
    }

    #[test]
    fn test_parse_token_invalid_prefix() {
        let config = Config {
            token_index_key: "secret".to_string(),
            ..Default::default()
        };
        assert!(parse_token("not_cms_", &config).is_err());
    }

    #[test]
    fn test_parse_token_too_short() {
        let config = Config {
            token_index_key: "secret".to_string(),
            ..Default::default()
        };
        assert!(parse_token("cms_", &config).is_err());
    }
}
