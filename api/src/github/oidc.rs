use anyhow::ensure;
use openidconnect::{
    AdditionalClaims, ClaimsVerificationError, ClientId, IssuerUrl, JsonWebKeySetUrl,
    NonceVerifier,
    core::{
        CoreGenderClaim, CoreJsonWebKey, CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm,
    },
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub struct IdTokenVerifier(openidconnect::IdTokenVerifier<'static, CoreJsonWebKey>);

impl IdTokenVerifier {
    pub async fn new(client_id: String) -> anyhow::Result<Self> {
        let client = reqwest::ClientBuilder::new()
            .user_agent(super::USER_AGENT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        let issuer = IssuerUrl::new("https://token.actions.githubusercontent.com".into()).unwrap();

        #[derive(Deserialize)]
        struct ProviderMetadata {
            issuer: IssuerUrl,
            jwks_uri: JsonWebKeySetUrl,
            id_token_signing_alg_values_supported: Vec<CoreJwsSigningAlgorithm>,
        }

        let provider_metadata = client
            .get(issuer.join(".well-known/openid-configuration").unwrap())
            .header("accept", "application/json")
            .send()
            .await?
            .error_for_status()?
            .json::<ProviderMetadata>()
            .await?;

        ensure!(
            provider_metadata.issuer == issuer,
            "unexpected issuer URI `{}` (expected `{issuer}`)",
            provider_metadata.issuer,
        );

        let jwks = client
            .get(&*provider_metadata.jwks_uri)
            .header("accept", "application/json")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(IdTokenVerifier(
            openidconnect::IdTokenVerifier::new_public_client(
                ClientId::new(client_id),
                issuer,
                jwks,
            )
            .set_allowed_algs(provider_metadata.id_token_signing_alg_values_supported),
        ))
    }

    pub fn verify(&self, id_token: &str) -> Result<Claims, IdTokenVerifyError> {
        Ok(id_token
            .parse::<IdToken>()?
            .into_claims(&self.0, &IgnoreNonceVerifier)?
            .additional_claims()
            .clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub actor: String,
    #[serde(deserialize_with = "crate::serde_ext::from_string")]
    pub actor_id: u64,
    pub repository: String,
    #[serde(deserialize_with = "crate::serde_ext::from_string")]
    pub run_attempt: u64,
    #[serde(deserialize_with = "crate::serde_ext::from_string")]
    pub run_id: u64,
    pub workflow: String,
    pub workflow_sha: String,
}

impl Claims {
    pub fn logs_url(&self) -> String {
        format!(
            "https://github.com/{}/actions/runs/{}/attempts/{}",
            self.repository, self.run_id, self.run_attempt
        )
    }
}

impl AdditionalClaims for Claims {}

#[derive(Debug, Error)]
pub enum IdTokenVerifyError {
    #[error("failed to parse: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("verification failed: {0}")]
    Verify(#[from] ClaimsVerificationError),
}

type IdToken = openidconnect::IdToken<
    Claims,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
>;

struct IgnoreNonceVerifier;

impl NonceVerifier for &IgnoreNonceVerifier {
    fn verify(self, _nonce: Option<&openidconnect::Nonce>) -> Result<(), String> {
        Ok(())
    }
}
