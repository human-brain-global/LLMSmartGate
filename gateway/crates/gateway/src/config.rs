//! Application configuration loaded from environment variables and config files.
//!
//! Configuration is loaded from environment variables with the `LLMSMARTGATE_` prefix
//! for gateway-specific settings. `DATABASE_URL` and `VALKEY_URL` are loaded without
//! prefix for compatibility with standard tooling.
//!
//! Secrets (database URL, Valkey URL, JWT secret) are masked in Debug output to
//! prevent accidental leakage in logs.

use std::env;
use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

/// Wraps a secret string, masking its value in Debug output.
#[derive(Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    /// Returns the inner secret value.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Create a Secret for testing purposes.
    #[cfg(test)]
    pub(crate) fn for_test(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Errors that can occur during configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A required environment variable is missing.
    #[error("required environment variable `{name}` is not set")]
    MissingRequired { name: String },

    /// An environment variable has an invalid value.
    #[error("invalid value for `{name}`: {reason}")]
    InvalidValue { name: String, reason: String },
}

/// Top-level gateway configuration.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub auth: AuthConfig,
    pub provider: ProviderConfig,
    pub observability: ObservabilityConfig,
}

/// Server binding configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
}

/// PostgreSQL database configuration.
#[derive(Clone)]
pub struct DatabaseConfig {
    pub url: Secret,
    pub pool_size: u32,
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("url", &self.url)
            .field("pool_size", &self.pool_size)
            .finish()
    }
}

/// Redis / Valkey cache configuration.
#[derive(Clone)]
pub struct RedisConfig {
    pub url: Secret,
    pub pool_size: u32,
}

impl fmt::Debug for RedisConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisConfig")
            .field("url", &self.url)
            .field("pool_size", &self.pool_size)
            .finish()
    }
}

/// Authentication and signing configuration.
#[derive(Clone)]
pub struct AuthConfig {
    pub admin_jwt_secret: Secret,
    pub timestamp_skew_secs: u64,
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field("admin_jwt_secret", &self.admin_jwt_secret)
            .field("timestamp_skew_secs", &self.timestamp_skew_secs)
            .finish()
    }
}

/// Provider communication configuration.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub timeout_ms: u64,
}

/// Observability (logging, metrics, tracing) configuration.
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub log_level: String,
    pub otel_endpoint: Option<String>,
    pub service_name: String,
}

impl Default for GatewayConfig {
    /// Creates a config with sensible development defaults (for testing).
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                port: DEFAULT_PORT,
            },
            database: DatabaseConfig {
                url: Secret("postgres://localhost/test".to_owned()),
                pool_size: DEFAULT_PG_POOL_SIZE,
            },
            redis: RedisConfig {
                url: Secret("redis://localhost:6379".to_owned()),
                pool_size: DEFAULT_VALKEY_POOL_SIZE,
            },
            auth: AuthConfig {
                admin_jwt_secret: Secret("test-secret".to_owned()),
                timestamp_skew_secs: DEFAULT_TIMESTAMP_SKEW_SECS,
            },
            provider: ProviderConfig {
                timeout_ms: DEFAULT_PROVIDER_TIMEOUT_MS,
            },
            observability: ObservabilityConfig {
                log_level: DEFAULT_LOG_LEVEL.to_owned(),
                otel_endpoint: None,
                service_name: DEFAULT_SERVICE_NAME.to_owned(),
            },
        }
    }
}

// --- Default constants ---

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_PG_POOL_SIZE: u32 = 20;
const DEFAULT_VALKEY_POOL_SIZE: u32 = 10;
const DEFAULT_TIMESTAMP_SKEW_SECS: u64 = 300;
const DEFAULT_PROVIDER_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_SERVICE_NAME: &str = "llmsmartgate";

/// A source of configuration values, abstracting over environment variables
/// to enable testing without mutating process state.
trait ConfigSource {
    fn get(&self, name: &str) -> Option<String>;
}

/// Reads configuration from actual environment variables.
struct EnvSource;

impl ConfigSource for EnvSource {
    fn get(&self, name: &str) -> Option<String> {
        env::var(name).ok()
    }
}

/// Reads a required value from the source, returning `ConfigError::MissingRequired` if absent.
fn required(source: &dyn ConfigSource, name: &str) -> Result<String, ConfigError> {
    source
        .get(name)
        .ok_or_else(|| ConfigError::MissingRequired {
            name: name.to_owned(),
        })
}

/// Reads an optional value from the source with a fallback default.
fn optional(source: &dyn ConfigSource, name: &str, default: &str) -> String {
    source.get(name).unwrap_or_else(|| default.to_owned())
}

/// Parses a numeric value from the source with a default.
fn parse_val<T>(source: &dyn ConfigSource, name: &str, default: T) -> Result<T, ConfigError>
where
    T: FromStr + fmt::Display,
    T::Err: fmt::Display,
{
    match source.get(name) {
        Some(val) => val.parse::<T>().map_err(|e| ConfigError::InvalidValue {
            name: name.to_owned(),
            reason: e.to_string(),
        }),
        None => Ok(default),
    }
}

/// Internal constructor that accepts any `ConfigSource`.
fn load_from(source: &dyn ConfigSource) -> Result<GatewayConfig, ConfigError> {
    let database_url = required(source, "DATABASE_URL")?;
    let valkey_url = required(source, "VALKEY_URL")?;
    let admin_jwt_secret = required(source, "LLMSMARTGATE_ADMIN_JWT_SECRET")?;

    let host_str = optional(source, "LLMSMARTGATE_HOST", DEFAULT_HOST);
    let host = host_str
        .parse::<IpAddr>()
        .map_err(|e| ConfigError::InvalidValue {
            name: "LLMSMARTGATE_HOST".to_owned(),
            reason: e.to_string(),
        })?;

    let port: u16 = parse_val(source, "LLMSMARTGATE_PORT", DEFAULT_PORT)?;
    let pg_pool_size: u32 = parse_val(source, "LLMSMARTGATE_PG_POOL_SIZE", DEFAULT_PG_POOL_SIZE)?;
    let valkey_pool_size: u32 = parse_val(
        source,
        "LLMSMARTGATE_VALKEY_POOL_SIZE",
        DEFAULT_VALKEY_POOL_SIZE,
    )?;
    let timestamp_skew_secs: u64 = parse_val(
        source,
        "LLMSMARTGATE_TIMESTAMP_SKEW_SECS",
        DEFAULT_TIMESTAMP_SKEW_SECS,
    )?;
    let provider_timeout_ms: u64 = parse_val(
        source,
        "LLMSMARTGATE_PROVIDER_TIMEOUT_MS",
        DEFAULT_PROVIDER_TIMEOUT_MS,
    )?;

    let log_level = optional(source, "RUST_LOG", DEFAULT_LOG_LEVEL);
    let otel_endpoint = source.get("OTEL_EXPORTER_OTLP_ENDPOINT");
    let service_name = optional(source, "OTEL_SERVICE_NAME", DEFAULT_SERVICE_NAME);

    Ok(GatewayConfig {
        server: ServerConfig { host, port },
        database: DatabaseConfig {
            url: Secret(database_url),
            pool_size: pg_pool_size,
        },
        redis: RedisConfig {
            url: Secret(valkey_url),
            pool_size: valkey_pool_size,
        },
        auth: AuthConfig {
            admin_jwt_secret: Secret(admin_jwt_secret),
            timestamp_skew_secs,
        },
        provider: ProviderConfig {
            timeout_ms: provider_timeout_ms,
        },
        observability: ObservabilityConfig {
            log_level,
            otel_endpoint,
            service_name,
        },
    })
}

impl GatewayConfig {
    /// Load configuration from environment variables.
    ///
    /// Required variables:
    /// - `DATABASE_URL` -- PostgreSQL connection string
    /// - `VALKEY_URL` -- Redis/Valkey connection string
    /// - `LLMSMARTGATE_ADMIN_JWT_SECRET` -- JWT signing secret for admin API
    ///
    /// Optional (with defaults):
    /// - `LLMSMARTGATE_HOST` (default: `0.0.0.0`)
    /// - `LLMSMARTGATE_PORT` (default: `8080`)
    /// - `LLMSMARTGATE_PG_POOL_SIZE` (default: `20`)
    /// - `LLMSMARTGATE_VALKEY_POOL_SIZE` (default: `10`)
    /// - `LLMSMARTGATE_TIMESTAMP_SKEW_SECS` (default: `300`)
    /// - `LLMSMARTGATE_PROVIDER_TIMEOUT_MS` (default: `30000`)
    /// - `RUST_LOG` (default: `info`)
    /// - `OTEL_EXPORTER_OTLP_ENDPOINT` (optional, no default)
    /// - `OTEL_SERVICE_NAME` (default: `llmsmartgate`)
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::MissingRequired`] if a required variable is not set,
    /// or [`ConfigError::InvalidValue`] if a variable contains an unparseable value.
    pub fn load() -> Result<Self, ConfigError> {
        load_from(&EnvSource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A test config source backed by a `HashMap`.
    struct MapSource(HashMap<String, String>);

    impl MapSource {
        fn new() -> Self {
            Self(HashMap::new())
        }

        fn set(mut self, key: &str, value: &str) -> Self {
            self.0.insert(key.to_owned(), value.to_owned());
            self
        }
    }

    impl ConfigSource for MapSource {
        fn get(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }
    }

    /// Returns a `MapSource` with all required and optional fields set.
    fn full_source() -> MapSource {
        MapSource::new()
            .set("DATABASE_URL", "postgres://test:test@localhost:5432/testdb")
            .set("VALKEY_URL", "redis://localhost:6379")
            .set("LLMSMARTGATE_ADMIN_JWT_SECRET", "test-jwt-secret")
            .set("LLMSMARTGATE_HOST", "127.0.0.1")
            .set("LLMSMARTGATE_PORT", "9090")
            .set("LLMSMARTGATE_PG_POOL_SIZE", "5")
            .set("LLMSMARTGATE_VALKEY_POOL_SIZE", "3")
            .set("LLMSMARTGATE_TIMESTAMP_SKEW_SECS", "600")
            .set("LLMSMARTGATE_PROVIDER_TIMEOUT_MS", "15000")
            .set("RUST_LOG", "debug")
            .set("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4317")
            .set("OTEL_SERVICE_NAME", "test-gateway")
    }

    /// Returns a `MapSource` with only required fields set.
    fn required_only_source() -> MapSource {
        MapSource::new()
            .set("DATABASE_URL", "postgres://test:test@localhost:5432/testdb")
            .set("VALKEY_URL", "redis://localhost:6379")
            .set("LLMSMARTGATE_ADMIN_JWT_SECRET", "test-jwt-secret")
    }

    #[test]
    fn load_config_with_all_env_vars() {
        let source = full_source();
        let config = load_from(&source).expect("should load config successfully");

        assert_eq!(
            config.server.host,
            "127.0.0.1".parse::<IpAddr>().expect("valid ip")
        );
        assert_eq!(config.server.port, 9090);
        assert_eq!(
            config.database.url.expose(),
            "postgres://test:test@localhost:5432/testdb"
        );
        assert_eq!(config.database.pool_size, 5);
        assert_eq!(config.redis.url.expose(), "redis://localhost:6379");
        assert_eq!(config.redis.pool_size, 3);
        assert_eq!(config.auth.admin_jwt_secret.expose(), "test-jwt-secret");
        assert_eq!(config.auth.timestamp_skew_secs, 600);
        assert_eq!(config.provider.timeout_ms, 15_000);
        assert_eq!(config.observability.log_level, "debug");
        assert_eq!(
            config.observability.otel_endpoint.as_deref(),
            Some("http://localhost:4317")
        );
        assert_eq!(config.observability.service_name, "test-gateway");
    }

    #[test]
    fn load_config_defaults_when_optional_vars_missing() {
        let source = required_only_source();
        let config = load_from(&source).expect("should load config with defaults");

        assert_eq!(
            config.server.host,
            "0.0.0.0".parse::<IpAddr>().expect("valid ip")
        );
        assert_eq!(config.server.port, DEFAULT_PORT);
        assert_eq!(config.database.pool_size, DEFAULT_PG_POOL_SIZE);
        assert_eq!(config.redis.pool_size, DEFAULT_VALKEY_POOL_SIZE);
        assert_eq!(config.auth.timestamp_skew_secs, DEFAULT_TIMESTAMP_SKEW_SECS);
        assert_eq!(config.provider.timeout_ms, DEFAULT_PROVIDER_TIMEOUT_MS);
        assert_eq!(config.observability.log_level, DEFAULT_LOG_LEVEL);
        assert!(config.observability.otel_endpoint.is_none());
        assert_eq!(config.observability.service_name, DEFAULT_SERVICE_NAME);
    }

    #[test]
    fn missing_database_url_returns_error() {
        let source = MapSource::new()
            .set("VALKEY_URL", "redis://localhost:6379")
            .set("LLMSMARTGATE_ADMIN_JWT_SECRET", "test-jwt-secret");

        let result = load_from(&source);
        assert!(result.is_err());

        let err = result.expect_err("should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("DATABASE_URL"),
            "error should mention DATABASE_URL, got: {msg}"
        );
    }

    #[test]
    fn missing_valkey_url_returns_error() {
        let source = MapSource::new()
            .set("DATABASE_URL", "postgres://test:test@localhost:5432/testdb")
            .set("LLMSMARTGATE_ADMIN_JWT_SECRET", "test-jwt-secret");

        let result = load_from(&source);
        assert!(result.is_err());

        let err = result.expect_err("should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("VALKEY_URL"),
            "error should mention VALKEY_URL, got: {msg}"
        );
    }

    #[test]
    fn missing_admin_jwt_secret_returns_error() {
        let source = MapSource::new()
            .set("DATABASE_URL", "postgres://test:test@localhost:5432/testdb")
            .set("VALKEY_URL", "redis://localhost:6379");

        let result = load_from(&source);
        assert!(result.is_err());

        let err = result.expect_err("should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("LLMSMARTGATE_ADMIN_JWT_SECRET"),
            "error should mention LLMSMARTGATE_ADMIN_JWT_SECRET, got: {msg}"
        );
    }

    #[test]
    fn secrets_not_exposed_in_debug_output() {
        let source = full_source();
        let config = load_from(&source).expect("should load config successfully");
        let debug_output = format!("{config:?}");

        // Actual secret values must NOT appear in debug output
        assert!(
            !debug_output.contains("postgres://test:test@localhost:5432/testdb"),
            "database URL should be redacted in Debug output"
        );
        assert!(
            !debug_output.contains("redis://localhost:6379"),
            "Valkey URL should be redacted in Debug output"
        );
        assert!(
            !debug_output.contains("test-jwt-secret"),
            "JWT secret should be redacted in Debug output"
        );

        // The redaction marker should appear instead
        assert!(
            debug_output.contains("[REDACTED]"),
            "Debug output should contain [REDACTED] markers"
        );
    }

    #[test]
    fn invalid_port_returns_error() {
        let source = required_only_source().set("LLMSMARTGATE_PORT", "not-a-number");

        let result = load_from(&source);
        assert!(result.is_err());

        let err = result.expect_err("should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("LLMSMARTGATE_PORT"),
            "error should mention LLMSMARTGATE_PORT, got: {msg}"
        );
    }

    #[test]
    fn invalid_host_returns_error() {
        let source = required_only_source().set("LLMSMARTGATE_HOST", "not-an-ip");

        let result = load_from(&source);
        assert!(result.is_err());

        let err = result.expect_err("should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("LLMSMARTGATE_HOST"),
            "error should mention LLMSMARTGATE_HOST, got: {msg}"
        );
    }

    #[test]
    fn secret_display_is_redacted() {
        let secret = Secret("my-secret-value".to_owned());
        assert_eq!(format!("{secret}"), "[REDACTED]");
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert_eq!(secret.expose(), "my-secret-value");
    }
}
