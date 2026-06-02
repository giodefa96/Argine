//! Runtime configuration, loaded from environment variables.
//!
//! Enforces the security baseline (SECURITY.md §3): the process refuses to
//! start in non-local environments if a secret is still the `changethis`
//! placeholder.

use std::env;

const PLACEHOLDER: &str = "changethis";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Local,
    Staging,
    Production,
}

impl Environment {
    /// Parse strictly: an unrecognized value is an error, not a silent fallback to
    /// `Local` (which would be fail-open — a typo like `prod` must not disable the
    /// non-local secret check).
    fn parse(s: &str) -> Result<Self, ConfigError> {
        match s {
            "local" => Ok(Environment::Local),
            "staging" => Ok(Environment::Staging),
            "production" => Ok(Environment::Production),
            other => Err(ConfigError::UnknownEnvironment(other.to_string())),
        }
    }

    fn is_local(self) -> bool {
        matches!(self, Environment::Local)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    pub bind_addr: String,
    pub secret_key: String,
    /// Explicit CORS origin allowlist (never `*` in production).
    pub cors_origins: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0} is still the default placeholder \"changethis\" — set a real value (see SECURITY.md §3)")]
    DefaultSecret(&'static str),
    #[error("unknown ENVIRONMENT \"{0}\" — expected local|staging|production")]
    UnknownEnvironment(String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment =
            Environment::parse(&env::var("ENVIRONMENT").unwrap_or_else(|_| "local".to_string()))?;
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let secret_key = env::var("SECRET_KEY").unwrap_or_else(|_| PLACEHOLDER.to_string());
        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let config = Config {
            environment,
            bind_addr,
            secret_key,
            cors_origins,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if !self.environment.is_local() && self.secret_key == PLACEHOLDER {
            return Err(ConfigError::DefaultSecret("SECRET_KEY"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(environment: Environment, secret: &str) -> Config {
        Config {
            environment,
            bind_addr: "0.0.0.0:8080".to_string(),
            secret_key: secret.to_string(),
            cors_origins: vec![],
        }
    }

    #[test]
    fn parse_known_environments() {
        assert_eq!(Environment::parse("local").unwrap(), Environment::Local);
        assert_eq!(Environment::parse("staging").unwrap(), Environment::Staging);
        assert_eq!(
            Environment::parse("production").unwrap(),
            Environment::Production
        );
    }

    #[test]
    fn parse_unknown_environment_is_error() {
        // fail-closed: a typo must not silently become Local
        assert!(matches!(
            Environment::parse("prod"),
            Err(ConfigError::UnknownEnvironment(_))
        ));
    }

    #[test]
    fn default_secret_allowed_only_in_local() {
        assert!(config_with(Environment::Local, PLACEHOLDER)
            .validate()
            .is_ok());
        assert!(matches!(
            config_with(Environment::Production, PLACEHOLDER).validate(),
            Err(ConfigError::DefaultSecret(_))
        ));
        assert!(matches!(
            config_with(Environment::Staging, PLACEHOLDER).validate(),
            Err(ConfigError::DefaultSecret(_))
        ));
    }

    #[test]
    fn real_secret_ok_in_production() {
        assert!(config_with(Environment::Production, "a-real-secret")
            .validate()
            .is_ok());
    }
}
