//! Configuration du service, lue depuis l'environnement.
//!
//! Toutes les valeurs ont un defaut sur pour le developpement local, sauf
//! `MPACER_SESSION_SECRET` et les identifiants Google, qui doivent etre fournis
//! en production (le demarrage echoue explicitement si `MPACER_ENV=production`
//! et qu'ils manquent).

use std::time::Duration;

/// Configuration de la base de donnees.
///
/// Deux formes acceptees :
/// * `MPACER_DATABASE_URL` : URL complete (tests, serveur externe) ;
/// * variables separees (`MPACER_DB_HOST`, `MPACER_DB_PORT`, `MPACER_DB_NAME`,
///   `MPACER_DB_USER`, `MPACER_DB_PASSWORD`, `MPACER_DB_SSLMODE`), ce qui
///   correspond exactement aux cles du secret applicatif genere par
///   CloudNativePG et evite tout probleme d'encodage dans l'URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseConfig {
    Url(String),
    Parts {
        host: String,
        port: u16,
        database: String,
        username: String,
        password: String,
        sslmode: String,
    },
}

impl DatabaseConfig {
    pub fn from_env() -> Self {
        if let Some(url) = env_var("MPACER_DATABASE_URL") {
            return DatabaseConfig::Url(url);
        }
        DatabaseConfig::Parts {
            host: env_var("MPACER_DB_HOST").unwrap_or_else(|| "localhost".to_string()),
            port: env_var("MPACER_DB_PORT")
                .and_then(|value| value.parse().ok())
                .unwrap_or(5432),
            database: env_var("MPACER_DB_NAME").unwrap_or_else(|| "mpacer".to_string()),
            username: env_var("MPACER_DB_USER").unwrap_or_else(|| "mpacer".to_string()),
            password: env_var("MPACER_DB_PASSWORD").unwrap_or_else(|| "mpacer".to_string()),
            // En cluster, PostgreSQL est joint par le reseau interne : TLS desactive
            // par defaut. Passer a `require` (avec le CA monte) si necessaire.
            sslmode: env_var("MPACER_DB_SSLMODE").unwrap_or_else(|| "disable".to_string()),
        }
    }

    /// URL de connexion PostgreSQL, correctement encodee.
    pub fn url(&self) -> String {
        match self {
            DatabaseConfig::Url(url) => url.clone(),
            DatabaseConfig::Parts {
                host,
                port,
                database,
                username,
                password,
                sslmode,
            } => {
                format!(
                    "postgresql://{}:{}@{}:{}/{}?sslmode={}",
                    encode(username),
                    encode(password),
                    host,
                    port,
                    encode(database),
                    sslmode
                )
            }
        }
    }

    /// Version lisible pour les journaux (sans mot de passe).
    pub fn redacted(&self) -> String {
        match self {
            DatabaseConfig::Url(url) => redact_url(url),
            DatabaseConfig::Parts {
                host,
                port,
                database,
                username,
                ..
            } => {
                format!("postgresql://{username}@{host}:{port}/{database}")
            }
        }
    }
}

/// Encodage des composants d'URL (RFC 3986) : un mot de passe peut contenir
/// des caracteres reserves.
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Masque le mot de passe d'une URL pour l'affichage.
fn redact_url(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(scheme_end), Some(at)) if at > scheme_end => {
            format!("{}://***{}", &url[..scheme_end], &url[at..])
        }
        _ => url.to_string(),
    }
}

/// Environnement d'execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

impl Environment {
    pub fn is_production(self) -> bool {
        self == Environment::Production
    }
}

/// Configuration complete du service.
#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    pub bind: String,
    pub database: DatabaseConfig,
    /// URL publique du service (utilisee pour les redirections OAuth).
    pub public_url: String,
    pub session_secret: String,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    /// Autorise une connexion de test sans Google (`/auth/dev-login`).
    pub dev_auth: bool,
    /// Cookie `Secure` : vrai en production (HTTPS), faux en HTTP local.
    pub cookie_secure: bool,
    pub device_code_ttl: Duration,
    pub session_ttl: Duration,
    pub token_ttl_days: i64,
}

impl Config {
    /// Construit la configuration depuis l'environnement.
    pub fn from_env() -> anyhow::Result<Self> {
        let environment = match env_var("MPACER_ENV").as_deref() {
            Some("production") | Some("prod") => Environment::Production,
            _ => Environment::Development,
        };
        let public_url =
            env_var("MPACER_PUBLIC_URL").unwrap_or_else(|| "http://localhost:8080".to_string());
        let public_url = public_url.trim_end_matches('/').to_string();
        let session_secret = env_var("MPACER_SESSION_SECRET")
            .unwrap_or_else(|| "dev-secret-a-remplacer".to_string());
        let google_client_id = env_var("MPACER_GOOGLE_CLIENT_ID");
        let google_client_secret = env_var("MPACER_GOOGLE_CLIENT_SECRET");

        if environment.is_production() {
            if session_secret == "dev-secret-a-remplacer" || session_secret.len() < 32 {
                anyhow::bail!(
                    "MPACER_SESSION_SECRET doit etre defini (>= 32 caracteres) en production"
                );
            }
            if google_client_id.is_none() || google_client_secret.is_none() {
                anyhow::bail!("MPACER_GOOGLE_CLIENT_ID et MPACER_GOOGLE_CLIENT_SECRET sont obligatoires en production");
            }
        }

        let cookie_secure = env_var("MPACER_COOKIE_SECURE")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(environment.is_production());

        Ok(Self {
            environment,
            bind: env_var("MPACER_BIND").unwrap_or_else(|| "0.0.0.0:8080".to_string()),
            database: DatabaseConfig::from_env(),
            public_url,
            session_secret,
            google_client_id,
            google_client_secret,
            dev_auth: env_var("MPACER_DEV_AUTH")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            cookie_secure,
            device_code_ttl: Duration::from_secs(600),
            session_ttl: Duration::from_secs(60 * 60 * 24 * 30),
            token_ttl_days: 365,
        })
    }

    /// URL de redirection OAuth declaree dans la console Google.
    pub fn google_redirect_uri(&self) -> String {
        format!("{}/auth/google/callback", self.public_url)
    }

    pub fn google_configured(&self) -> bool {
        self.google_client_id.is_some() && self.google_client_secret.is_some()
    }
}

impl Config {
    /// Configuration minimale pour les tests d'integration (aucun acces reseau).
    pub fn for_tests(public_url: &str, database_url: &str) -> Self {
        Self {
            environment: Environment::Development,
            bind: "127.0.0.1:0".to_string(),
            database: DatabaseConfig::Url(database_url.to_string()),
            public_url: public_url.trim_end_matches('/').to_string(),
            session_secret: "secret-de-test-suffisamment-long-pour-hs256".to_string(),
            google_client_id: Some("client-de-test".to_string()),
            google_client_secret: Some("secret-de-test".to_string()),
            dev_auth: false,
            cookie_secure: false,
            device_code_ttl: Duration::from_secs(600),
            session_ttl: Duration::from_secs(3600),
            token_ttl_days: 365,
        }
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}
