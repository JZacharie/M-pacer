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
/// Instance Deemix par defaut, utilisee quand `MPACER_DEEMIX_URL` est vide.
pub const DEFAULT_DEEMIX_URL: &str = "https://deemix.p.zacharie.org";

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
    /// Identifiants de l'application Deezer (absents = source desactivee).
    ///
    /// Deezer exige l'app_id **et** le secret applicatif pour echanger le code
    /// d'autorisation : sans les deux, la source reste eteinte et la page
    /// /music continue de fonctionner avec Spotify ou un dossier local.
    pub deezer_app_id: Option<String>,
    pub deezer_app_secret: Option<String>,
    /// URI de redirection Deezer explicitement enregistree.
    /// Vide => `{public_url}/auth/deezer/callback`.
    pub deezer_redirect_uri: Option<String>,
    /// Cookie `arl` du compte Deezer (API privee `gw-light`).
    ///
    /// C'est la voie recommandee : elle ne demande **aucune** application
    /// developpeur Deezer et donne acces aux playlists du compte, y compris
    /// celles qui ne sont pas publiques. Le mode OAuth (`deezer_app_id` +
    /// `deezer_app_secret`) reste accepte en repli.
    pub deezer_arl: Option<String>,
    /// Base de l'instance Deemix qui telecharge les MP3.
    /// Vide => `https://deemix.p.zacharie.org`.
    pub deemix_url: Option<String>,
    /// Identifiants de l'authentification HTTP basique de Deemix (Traefik).
    /// Sans les deux, l'API n'est pas interrogee : seuls les liens restent.
    pub deemix_user: Option<String>,
    pub deemix_password: Option<String>,
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
            deezer_app_id: env_var("MPACER_DEEZER_APP_ID"),
            deezer_app_secret: env_var("MPACER_DEEZER_APP_SECRET"),
            deezer_redirect_uri: env_var("MPACER_DEEZER_REDIRECT_URI"),
            deezer_arl: env_var("MPACER_DEEZER_ARL"),
            deemix_url: env_var("MPACER_DEEMIX_URL"),
            deemix_user: env_var("MPACER_DEEMIX_USER"),
            deemix_password: env_var("MPACER_DEEMIX_PASSWORD"),
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

    /// URI de redirection OAuth declaree dans la console Deezer.
    pub fn deezer_redirect_uri(&self) -> String {
        self.deezer_redirect_uri
            .clone()
            .unwrap_or_else(|| format!("{}/auth/deezer/callback", self.public_url))
    }

    /// Chemin (sur ce service) ou Deezer renverra le navigateur.
    pub fn deezer_redirect_path(&self) -> String {
        redirect_path_of(&self.deezer_redirect_uri())
    }

    /// Vrai si une application Deezer **exploitable** est configuree (mode OAuth).
    pub fn deezer_app_configured(&self) -> bool {
        match (&self.deezer_app_id, &self.deezer_app_secret) {
            (Some(id), Some(secret)) => {
                !looks_like_placeholder(id) && !looks_like_placeholder(secret)
            }
            _ => false,
        }
    }

    /// Vrai si un cookie `arl` exploitable est configure (mode recommande).
    ///
    /// Un `arl` Deezer fait plusieurs dizaines de caracteres : une valeur trop
    /// courte est un gabarit laisse en place, pas un vrai cookie.
    pub fn deezer_arl_configured(&self) -> bool {
        self.deezer_arl
            .as_deref()
            .map(str::trim)
            .is_some_and(|arl| arl.len() >= 32 && !looks_like_placeholder(arl))
    }

    /// Vrai si la source Deezer est exploitable, par cookie `arl` **ou** par OAuth.
    pub fn deezer_configured(&self) -> bool {
        self.deezer_arl_configured() || self.deezer_app_configured()
    }

    /// Vrai si l'API de l'instance Deemix peut etre interrogee.
    ///
    /// Il faut l'authentification HTTP basique de l'instance (Traefik) **et** le
    /// cookie `arl` : Deemix ouvre sa session Deezer avec ce cookie a chaque
    /// envoi. Sans ces valeurs, la page `/music` se contente des liens.
    pub fn deemix_configured(&self) -> bool {
        let identifiants = match (&self.deemix_user, &self.deemix_password) {
            (Some(user), Some(password)) => {
                !looks_like_placeholder(user) && !looks_like_placeholder(password)
            }
            _ => false,
        };
        identifiants && self.deezer_arl_configured()
    }

    /// Base de l'instance Deemix qui telecharge les MP3.
    ///
    /// Les liens produits sont les routes de l'interface Deemix
    /// (`{base}/#/playlist/{id}`, `{base}/#/track/{id}`) : l'utilisateur les
    /// ouvre dans le navigateur ou il est deja authentifie sur son instance.
    pub fn deemix_base_url(&self) -> String {
        self.deemix_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .unwrap_or(DEFAULT_DEEMIX_URL)
            .trim_end_matches('/')
            .to_string()
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
            deezer_app_id: None,
            deezer_app_secret: None,
            deezer_redirect_uri: None,
            deezer_arl: None,
            deemix_url: None,
            deemix_user: None,
            deemix_password: None,
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
    fn deezer_is_optional_and_configurable() {
        let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert!(
            !config.deezer_configured(),
            "sans identifiants, Deezer est desactive"
        );
        assert_eq!(
            config.deezer_redirect_uri(),
            "http://localhost:8080/auth/deezer/callback"
        );
        assert_eq!(config.deezer_redirect_path(), "/auth/deezer/callback");

        // Un gabarit ne compte pas comme configure, et il faut les deux valeurs.
        config.deezer_app_id = Some("REMPLACER-PAR-VOTRE-APP-ID".to_string());
        config.deezer_app_secret = Some("vrai-secret".to_string());
        assert!(!config.deezer_configured());

        config.deezer_app_id = Some("123456".to_string());
        assert!(config.deezer_configured());
        config.deezer_app_secret = None;
        assert!(!config.deezer_configured(), "l'app_id seul ne suffit pas");

        config.deezer_app_secret = Some("vrai-secret".to_string());
        config.deezer_redirect_uri =
            Some("https://mpacer.p.zacharie.org/deezer/retour".to_string());
        assert_eq!(config.deezer_redirect_path(), "/deezer/retour");
        assert_eq!(
            config.deezer_redirect_uri(),
            "https://mpacer.p.zacharie.org/deezer/retour"
        );
    }

    #[test]
    fn deezer_arl_enables_the_source_without_a_developer_application() {
        let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert!(!config.deezer_configured());

        // Un gabarit trop court ne compte pas comme un cookie.
        config.deezer_arl = Some("REMPLACER-PAR-VOTRE-ARL".to_string());
        assert!(!config.deezer_arl_configured());
        assert!(!config.deezer_configured());

        config.deezer_arl = Some("a".repeat(64));
        assert!(config.deezer_arl_configured());
        assert!(config.deezer_configured(), "l'arl suffit, sans app Deezer");
        assert!(
            !config.deezer_app_configured(),
            "aucune application developpeur n'est necessaire"
        );
    }

    #[test]
    fn deemix_url_defaults_to_the_zacharie_instance() {
        let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert_eq!(config.deemix_base_url(), DEFAULT_DEEMIX_URL);

        // La barre oblique finale est retiree : les liens sont construits par
        // concatenation ("{base}/#/playlist/{id}").
        config.deemix_url = Some("https://deemix.exemple.org/".to_string());
        assert_eq!(config.deemix_base_url(), "https://deemix.exemple.org");

        config.deemix_url = Some("   ".to_string());
        assert_eq!(config.deemix_base_url(), DEFAULT_DEEMIX_URL);
    }

    #[test]
    fn deemix_api_needs_credentials_and_the_arl() {
        let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert!(!config.deemix_configured(), "rien de renseigne : pas d'API");

        // Les identifiants seuls ne suffisent pas : Deemix ouvre sa session
        // Deezer avec le cookie arl du service.
        config.deemix_user = Some("joseph".to_string());
        config.deemix_password = Some("mot-de-passe".to_string());
        assert!(!config.deemix_configured());

        config.deezer_arl = Some("a".repeat(64));
        assert!(config.deemix_configured());
        assert_eq!(config.deemix_base_url(), DEFAULT_DEEMIX_URL);

        // Un gabarit laisse en place ne compte pas comme identifiant.
        config.deemix_user = Some("REMPLACER-PAR-VOTRE-UTILISATEUR".to_string());
        assert!(!config.deemix_configured());
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
