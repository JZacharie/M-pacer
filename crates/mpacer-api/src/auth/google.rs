//! Connexion Google : OAuth 2.0 Authorization Code + PKCE, puis verification
//! de l'`id_token` contre les cles publiques de Google (JWKS).
//!
//! Le fournisseur est abstrait derriere `OidcProvider` : l'application reelle
//! (`GoogleOidc`) est utilisee en production, un faux fournisseur permet de tester
//! le flux complet sans dependre de Google.

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const AUTHORIZE_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const JWKS_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v3/certs";
const ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];

/// Identite renvoyee par le fournisseur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoogleUser {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub email_verified: bool,
}

/// Reponse de test pour `OidcProvider`, compatible avec les objets `dyn`.
pub type ExchangeFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<GoogleUser>> + Send + 'a>>;

/// Fournisseur d'identite OpenID Connect.
pub trait OidcProvider: Send + Sync {
    /// URL de consentement a laquelle rediriger l'utilisateur.
    fn authorize_url(&self, redirect_uri: &str, state: &str, code_challenge: &str) -> String;

    /// Echange le code d'autorisation contre l'identite de l'utilisateur.
    fn exchange<'a>(
        &'a self,
        code: &'a str,
        verifier: &'a str,
        redirect_uri: &'a str,
    ) -> ExchangeFuture<'a>;
}

/// Reponse du point d'echange de jeton.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
}

/// Claims de l'`id_token` Google.
///
/// `aud` et `iss` ne sont pas relus apres coup : ils sont verifies par
/// `jsonwebtoken::Validation` (audience et emetteur), ce qui est l'endroit correct
/// pour le faire.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    sub: String,
    email: String,
    #[serde(default)]
    email_verified: bool,
    name: Option<String>,
    picture: Option<String>,
    aud: String,
    iss: String,
}

/// Fournisseur Google reel.
#[derive(Clone)]
pub struct GoogleOidc {
    client_id: String,
    client_secret: String,
    http: reqwest::Client,
    jwks: Arc<RwLock<Option<(JwkSet, Instant)>>>,
}

impl GoogleOidc {
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            http,
            jwks: Arc::new(RwLock::new(None)),
        }
    }

    /// Recupere (et met en cache une heure) les cles publiques de Google.
    async fn jwks(&self) -> anyhow::Result<JwkSet> {
        if let Some((set, fetched_at)) = self.jwks.read().await.as_ref() {
            if fetched_at.elapsed() < Duration::from_secs(3600) {
                return Ok(set.clone());
            }
        }
        let set: JwkSet = self
            .http
            .get(JWKS_ENDPOINT)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        *self.jwks.write().await = Some((set.clone(), Instant::now()));
        Ok(set)
    }

    /// Verifie la signature et les claims d'un `id_token`.
    async fn verify_id_token(&self, id_token: &str) -> anyhow::Result<GoogleUser> {
        let header = decode_header(id_token)?;
        let kid = header
            .kid
            .ok_or_else(|| anyhow::anyhow!("id_token sans kid"))?;
        let jwks = self.jwks().await?;
        let jwk = jwks
            .keys
            .iter()
            .find(|key| key.common.key_id.as_deref() == Some(kid.as_str()))
            .ok_or_else(|| anyhow::anyhow!("cle publique inconnue : {kid}"))?;
        let key = DecodingKey::from_jwk(jwk)?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.client_id.as_str()]);
        validation.set_issuer(&ISSUERS);

        let data = decode::<IdTokenClaims>(id_token, &key, &validation)?;
        let claims = data.claims;
        if !claims.email_verified {
            anyhow::bail!("adresse e-mail Google non verifiee");
        }
        Ok(GoogleUser {
            sub: claims.sub,
            email: claims.email,
            name: claims.name,
            picture: claims.picture,
            email_verified: claims.email_verified,
        })
    }
}

impl OidcProvider for GoogleOidc {
    fn authorize_url(&self, redirect_uri: &str, state: &str, code_challenge: &str) -> String {
        let mut url = url::Url::parse(AUTHORIZE_ENDPOINT).expect("URL Google valide");
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", "openid email profile")
            .append_pair("state", state)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("access_type", "online")
            .append_pair("prompt", "select_account");
        url.to_string()
    }

    fn exchange<'a>(
        &'a self,
        code: &'a str,
        verifier: &'a str,
        redirect_uri: &'a str,
    ) -> ExchangeFuture<'a> {
        Box::pin(async move {
            let response = self
                .http
                .post(TOKEN_ENDPOINT)
                .form(&[
                    ("code", code),
                    ("client_id", self.client_id.as_str()),
                    ("client_secret", self.client_secret.as_str()),
                    ("redirect_uri", redirect_uri),
                    ("grant_type", "authorization_code"),
                    ("code_verifier", verifier),
                ])
                .send()
                .await?
                .error_for_status()?;
            let payload: TokenResponse = response.json().await?;
            let id_token = payload
                .id_token
                .ok_or_else(|| anyhow::anyhow!("reponse Google sans id_token"))?;
            self.verify_id_token(&id_token).await
        })
    }
}

/// Fournisseur de repli lorsque Google n'est pas configure (developpement).
pub struct UnconfiguredOidc;

impl OidcProvider for UnconfiguredOidc {
    fn authorize_url(&self, _redirect_uri: &str, _state: &str, _code_challenge: &str) -> String {
        "/login?erreur=google_non_configure".to_string()
    }

    fn exchange<'a>(
        &'a self,
        _code: &'a str,
        _verifier: &'a str,
        _redirect_uri: &'a str,
    ) -> ExchangeFuture<'a> {
        Box::pin(async { anyhow::bail!("Google OAuth n'est pas configure sur ce service") })
    }
}

/// Petit constructeur d'URL minimal (evite une dependance supplementaire).
mod url {
    /// Encodage `application/x-www-form-urlencoded` d'une valeur de requete.
    pub fn encode(value: &str) -> String {
        value
            .bytes()
            .map(|byte| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (byte as char).to_string()
                }
                b' ' => "+".to_string(),
                other => format!("%{other:02X}"),
            })
            .collect()
    }

    /// URL avec parametres de requete.
    pub struct Url {
        base: String,
        params: Vec<(String, String)>,
    }

    impl Url {
        pub fn parse(base: &str) -> Result<Self, ()> {
            if base.starts_with("https://") || base.starts_with("http://") {
                Ok(Url {
                    base: base.to_string(),
                    params: Vec::new(),
                })
            } else {
                Err(())
            }
        }

        pub fn query_pairs_mut(&mut self) -> &mut Self {
            &mut *self
        }

        pub fn append_pair(&mut self, key: &str, value: &str) -> &mut Self {
            self.params.push((key.to_string(), value.to_string()));
            self
        }
    }

    /// Rendu `Display` : URL complete avec ses parametres de requete.
    impl std::fmt::Display for Url {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let query = self
                .params
                .iter()
                .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
                .collect::<Vec<_>>()
                .join("&");
            write!(formatter, "{}?{}", self.base, query)
        }
    }
}
