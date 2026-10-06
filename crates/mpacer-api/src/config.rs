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
    /// URI de redirection OAuth explicitement enregistree dans la console Google.
    /// Vide => `{public_url}/auth/google/callback`.
    pub google_redirect_uri: Option<String>,
    /// Identifiants de l'application Spotify (absents = fonctionnalite desactivee).
    pub spotify_client_id: Option<String>,
    pub spotify_client_secret: Option<String>,
    /// URI de redirection Spotify explicitement enregistree dans la console.
    /// Vide => `{public_url}/auth/spotify/callback`.
    pub spotify_redirect_uri: Option<String>,
    /// Autorise une connexion de test sans Google (`/auth/dev-login`).
    pub dev_auth: bool,
    /// Broker MQTT du suivi en direct (absent = fonctionnalite eteinte, aucun cout).
    pub mqtt_url: Option<String>,
    /// Filtre de sujet souscrit sur le broker.
    pub mqtt_topic: String,
    pub mqtt_username: Option<String>,
    pub mqtt_password: Option<String>,
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
            google_redirect_uri: env_var("MPACER_GOOGLE_REDIRECT_URI"),
            spotify_client_id: env_var("MPACER_SPOTIFY_CLIENT_ID"),
            spotify_client_secret: env_var("MPACER_SPOTIFY_CLIENT_SECRET"),
            spotify_redirect_uri: env_var("MPACER_SPOTIFY_REDIRECT_URI"),
            dev_auth: env_var("MPACER_DEV_AUTH")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            mqtt_url: env_var("MPACER_MQTT_URL"),
            mqtt_topic: env_var("MPACER_MQTT_TOPIC")
                .unwrap_or_else(|| crate::live::DEFAULT_TOPIC.to_string()),
            mqtt_username: env_var("MPACER_MQTT_USERNAME"),
            mqtt_password: env_var("MPACER_MQTT_PASSWORD"),
            cookie_secure,
            device_code_ttl: Duration::from_secs(600),
            session_ttl: Duration::from_secs(60 * 60 * 24 * 30),
            token_ttl_days: 365,
        })
    }

    /// URL de redirection OAuth declaree dans la console Google.
    ///
    /// Un client OAuth peut avoir enregistre un chemin different (ex. `/Authorized`) :
    /// `MPACER_GOOGLE_REDIRECT_URI` permet de s'y aligner sans changer le code, et le
    /// routeur sert alors le callback a ce chemin (voir `google_redirect_path`).
    pub fn google_redirect_uri(&self) -> String {
        self.google_redirect_uri
            .clone()
            .unwrap_or_else(|| format!("{}/auth/google/callback", self.public_url))
    }

    /// Chemin (sur ce service) ou Google renverra le navigateur.
    pub fn google_redirect_path(&self) -> String {
        redirect_path_of(&self.google_redirect_uri())
    }

    /// URI de redirection OAuth declaree dans la console Spotify.
    pub fn spotify_redirect_uri(&self) -> String {
        self.spotify_redirect_uri
            .clone()
            .unwrap_or_else(|| format!("{}/auth/spotify/callback", self.public_url))
    }

    /// Chemin (sur ce service) ou Spotify renverra le navigateur.
    pub fn spotify_redirect_path(&self) -> String {
        redirect_path_of(&self.spotify_redirect_uri())
    }

    /// Vrai si un client Spotify **exploitable** est configure.
    ///
    /// Sans identifiants, la page /music reste utilisable (fichiers personnels)
    /// et annonce explicitement que Spotify n'est pas disponible.
    pub fn spotify_configured(&self) -> bool {
        match (&self.spotify_client_id, &self.spotify_client_secret) {
            (Some(id), Some(secret)) => {
                !looks_like_placeholder(id) && !looks_like_placeholder(secret)
            }
            _ => false,
        }
    }

    /// Vrai si le suivi en direct est configure.
    ///
    /// Sans `MPACER_MQTT_URL`, aucune tache n'est lancee : le service reste
    /// strictement identique a ce qu'il etait avant le suivi en direct.
    pub fn mqtt_configured(&self) -> bool {
        self.mqtt_url
            .as_ref()
            .is_some_and(|url| !url.trim().is_empty())
    }

    /// Vrai si un client Google **exploitable** est configure.
    ///
    /// Un identifiant laisse au gabarit (`REMPLACER-...`) est traite comme absent :
    /// le service reste sain, mais la page de connexion annonce explicitement que
    /// Google n'est pas configure au lieu d'envoyer l'utilisateur vers une erreur
    /// de la console Google.
    pub fn google_configured(&self) -> bool {
        match (&self.google_client_id, &self.google_client_secret) {
            (Some(id), Some(secret)) => {
                !looks_like_placeholder(id) && !looks_like_placeholder(secret)
            }
            _ => false,
        }
    }
}

/// Chemin d'une URI de redirection, sans hote ni parametres.
fn redirect_path_of(uri: &str) -> String {
    let after_scheme = uri.split_once("://").map(|(_, reste)| reste).unwrap_or(uri);
    match after_scheme.find('/') {
        Some(index) => {
            let path = after_scheme[index..]
                .split(['?', '#'])
                .next()
                .unwrap_or("/");
            if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            }
        }
        None => "/".to_string(),
    }
}

/// Detecte une valeur de gabarit oubliee dans la configuration.
fn looks_like_placeholder(value: &str) -> bool {
    const MARKERS: [&str; 6] = [
        "REMPLACER",
        "CHANGEME",
        "CHANGE_ME",
        "PLACEHOLDER",
        "VOTRE-CLIENT",
        "XXX",
    ];
    let upper = value.to_uppercase();
    MARKERS.iter().any(|marker| upper.contains(marker))
}

impl Config {
    /// Configuration minimale pour les tests d'integration (aucun acces reseau).
    #[allow(clippy::too_many_lines)]
    pub fn for_tests(public_url: &str, database_url: &str) -> Self {
        Self {
            environment: Environment::Development,
            bind: "127.0.0.1:0".to_string(),
            database: DatabaseConfig::Url(database_url.to_string()),
            public_url: public_url.trim_end_matches('/').to_string(),
            session_secret: "secret-de-test-suffisamment-long-pour-hs256".to_string(),
            google_client_id: Some("client-de-test".to_string()),
            google_client_secret: Some("secret-de-test".to_string()),
            google_redirect_uri: None,
            spotify_client_id: None,
            spotify_client_secret: None,
            spotify_redirect_uri: None,
            dev_auth: false,
            mqtt_url: None,
            mqtt_topic: crate::live::DEFAULT_TOPIC.to_string(),
            mqtt_username: None,
            mqtt_password: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_credentials_are_treated_as_unconfigured() {
        let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert!(
            config.google_configured(),
            "les valeurs de test sont exploitables"
        );

        config.google_client_id =
            Some("REMPLACER-PAR-VOTRE-CLIENT-ID.apps.googleusercontent.com".to_string());
        assert!(
            !config.google_configured(),
            "un gabarit ne doit pas passer pour configure"
        );

        config.google_client_id = Some("1234.apps.googleusercontent.com".to_string());
        config.google_client_secret = Some("CHANGEME".to_string());
        assert!(!config.google_configured());

        config.google_client_secret = Some("GOCSPX-vrai-secret".to_string());
        assert!(config.google_configured());
    }

    #[test]
    fn redirect_uri_and_path_are_configurable() {
        let mut config = Config::for_tests("https://mpacer.p.zacharie.org", "postgresql://exemple");
        assert_eq!(
            config.google_redirect_uri(),
            "https://mpacer.p.zacharie.org/auth/google/callback"
        );
        assert_eq!(config.google_redirect_path(), "/auth/google/callback");

        // Cas d'un client OAuth dont l'URI enregistree est differente
        config.google_redirect_uri = Some("https://mpacer.p.zacharie.org/Authorized".to_string());
        assert_eq!(config.google_redirect_path(), "/Authorized");
        assert_eq!(
            config.google_redirect_uri(),
            "https://mpacer.p.zacharie.org/Authorized"
        );
    }

    #[test]
    fn spotify_is_optional_and_configurable() {
        let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert!(
            !config.spotify_configured(),
            "sans identifiants, Spotify est desactive"
        );
        assert_eq!(
            config.spotify_redirect_uri(),
            "http://localhost:8080/auth/spotify/callback"
        );
        assert_eq!(config.spotify_redirect_path(), "/auth/spotify/callback");

        // Un identifiant laisse au gabarit ne compte pas comme configure.
        config.spotify_client_id = Some("REMPLACER-PAR-VOTRE-CLIENT-ID".to_string());
        config.spotify_client_secret = Some("vrai-secret".to_string());
        assert!(!config.spotify_configured());

        config.spotify_client_id = Some("vrai-client".to_string());
        assert!(config.spotify_configured());

        config.spotify_redirect_uri =
            Some("https://mpacer.p.zacharie.org/spotify/retour".to_string());
        assert_eq!(config.spotify_redirect_path(), "/spotify/retour");
        assert_eq!(
            config.spotify_redirect_uri(),
            "https://mpacer.p.zacharie.org/spotify/retour"
        );
    }

    #[test]
    fn live_tracking_is_optional() {
        let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert!(!config.mqtt_configured(), "aucun broker par defaut");
        assert_eq!(config.mqtt_topic, "mpacer/live/+");

        config.mqtt_url = Some("   ".to_string());
        assert!(!config.mqtt_configured(), "une adresse vide n'active rien");

        config.mqtt_url = Some("mqtt://broker.mpacer.svc:1883".to_string());
        assert!(config.mqtt_configured());
    }

    #[test]
    fn database_url_is_encoded_and_redacted() {
        let config = DatabaseConfig::Parts {
            host: "db.local".into(),
            port: 5432,
            database: "mpacer".into(),
            username: "mpacer".into(),
            password: "mot de passe/avec:caracteres".into(),
            sslmode: "disable".into(),
        };
        let url = config.url();
        assert!(url.starts_with(
            "postgresql://mpacer:mot%20de%20passe%2Favec%3Acaracteres@db.local:5432/mpacer"
        ));
        assert!(!config.redacted().contains("mot de passe"));
    }
}
