use crate::config::Config;
use crate::grpc::interceptor::compute_key_hmac;

/// Lightweight auth context extracted from the Bearer token.
/// This is inserted into request extensions by the interceptor.
#[derive(Clone, Debug)]
pub struct AuthContext {
    pub token: String,
    pub prefix: String,
    pub hmac: String,
}

/// Returned when a raw Bearer token fails format validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidToken;

/// Parse and validate the raw Bearer token format.
///
/// Returns an `AuthContext` on success, or `InvalidToken` on any validation failure.
pub fn parse_token(token: &str, config: &Config) -> Result<AuthContext, InvalidToken> {
    if !token.starts_with("vcms_site_") {
        return Err(InvalidToken);
    }

    let prefix = token.get(..24).ok_or(InvalidToken)?.to_string();
    let hmac = compute_key_hmac(token, &config.hmac_secret);

    Ok(AuthContext {
        token: token.to_string(),
        prefix,
        hmac,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_token_valid() {
        let config = Config {
            hmac_secret: "secret".to_string(),
            ..Default::default()
        };
        let token = "vcms_site_abc1234567890123456";
        let ctx = parse_token(token, &config).unwrap();
        assert_eq!(ctx.token, token);
        assert_eq!(ctx.prefix, token.chars().take(24).collect::<String>());
        assert_eq!(ctx.hmac.len(), 64);
    }

    #[test]
    fn test_parse_token_invalid_prefix() {
        let config = Config {
            hmac_secret: "secret".to_string(),
            ..Default::default()
        };
        assert!(parse_token("not_cms_", &config).is_err());
    }

    #[test]
    fn test_parse_token_too_short() {
        let config = Config {
            hmac_secret: "secret".to_string(),
            ..Default::default()
        };
        assert!(parse_token("cms_", &config).is_err());
    }
}
