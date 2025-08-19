#![allow(dead_code)]

use std::sync::OnceLock;

use openidconnect::{
    AuthenticationFlow, AuthorizationCode, Client, ClientId, ClientSecret, CsrfToken,
    EmptyAdditionalClaims, EmptyExtraTokenFields, EndpointMaybeSet, EndpointNotSet, EndpointSet,
    IdTokenFields, IssuerUrl, Nonce, RedirectUrl, Scope, StandardErrorResponse,
    StandardTokenResponse,
    core::{self, CoreClient, CoreResponseType},
};
use tracing::error;
use url::Url;

use crate::{
    config::CONFIG,
    errors::{BridgeError, Result},
};

#[allow(clippy::upper_case_acronyms)]
// LMFAO
type OIDC = Client<
    EmptyAdditionalClaims,
    core::CoreAuthDisplay,
    core::CoreGenderClaim,
    core::CoreJweContentEncryptionAlgorithm,
    core::CoreJsonWebKey,
    core::CoreAuthPrompt,
    StandardErrorResponse<core::CoreErrorResponseType>,
    StandardTokenResponse<
        IdTokenFields<
            EmptyAdditionalClaims,
            EmptyExtraTokenFields,
            core::CoreGenderClaim,
            core::CoreJweContentEncryptionAlgorithm,
            core::CoreJwsSigningAlgorithm,
        >,
        core::CoreTokenType,
    >,
    openidconnect::StandardTokenIntrospectionResponse<EmptyExtraTokenFields, core::CoreTokenType>,
    core::CoreRevocableToken,
    StandardErrorResponse<openidconnect::RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

type Token = StandardTokenResponse<
    IdTokenFields<
        EmptyAdditionalClaims,
        EmptyExtraTokenFields,
        core::CoreGenderClaim,
        core::CoreJweContentEncryptionAlgorithm,
        core::CoreJwsSigningAlgorithm,
    >,
    core::CoreTokenType,
>;

pub enum OpenIDProvider {
    W3,
    IbmId,
    None,
}

impl From<OpenIDProvider> for &'static str {
    fn from(provider: OpenIDProvider) -> Self {
        match provider {
            OpenIDProvider::W3 => "openid-w3",
            OpenIDProvider::IbmId => "openid-ibmid",
            OpenIDProvider::None => "",
        }
    }
}

impl From<&str> for OpenIDProvider {
    fn from(provider: &str) -> Self {
        match provider {
            "w3" => OpenIDProvider::W3,
            "ibm" => OpenIDProvider::IbmId,
            _ => OpenIDProvider::None,
        }
    }
}

pub fn get_openid_provider(provider: OpenIDProvider) -> Result<&'static OpenID> {
    match provider {
        OpenIDProvider::W3 => OPENID_W3.get(),
        OpenIDProvider::IbmId => OPENID_IBM.get(),
        OpenIDProvider::None => None,
    }
    .ok_or_else(|| BridgeError::AuthorizationServerNotSupported)
}

pub struct OpenID {
    client: OIDC,
    reqwest_client: reqwest::Client,
}

pub static OPENID_W3: OnceLock<OpenID> = OnceLock::new();
pub static OPENID_IBM: OnceLock<OpenID> = OnceLock::new();

impl OpenID {
    async fn new(table_name: OpenIDProvider) -> Result<Self> {
        let table_name: &str = table_name.into();
        let oidc = CONFIG
            .oidc
            .get(table_name)
            .ok_or_else(|| BridgeError::TomlLookupError)?;

        let reqwest_client = reqwest::Client::new();

        let provider_metadata = core::CoreProviderMetadata::discover_async(
            IssuerUrl::new(oidc.url.to_owned())?,
            &reqwest_client,
        )
        .await
        .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(oidc.client_id.to_string()),
            Some(ClientSecret::new(oidc.client_secret.to_string())),
        )
        .set_redirect_uri(
            RedirectUrl::new(oidc.redirect_url.to_string())
                .map_err(|e| BridgeError::GeneralError(e.to_string()))?,
        );

        Ok(OpenID {
            client,
            reqwest_client,
        })
    }

    pub fn get_client_resources(&self) -> (Url, CsrfToken, Nonce) {
        let (u, c, n) = self
            .client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();
        (u, c, n)
    }

    pub async fn get_token(&self, code: String) -> Result<Token> {
        let token = self
            .client
            .exchange_code(AuthorizationCode::new(code))?
            .request_async(&self.reqwest_client)
            .await
            .map_err(|e| BridgeError::TokenRequestError(e.to_string()))?;
        Ok(token)
    }

    pub fn get_verifier(&self) -> openidconnect::IdTokenVerifier<'_, core::CoreJsonWebKey> {
        self.client.id_token_verifier()
    }
}

pub async fn init_once() {
    if let (Ok(openidw3), Ok(openidibmid)) = (
        OpenID::new(OpenIDProvider::W3).await,
        OpenID::new(OpenIDProvider::IbmId).await,
    ) {
        OPENID_W3.get_or_init(|| openidw3);
        OPENID_IBM.get_or_init(|| openidibmid);
        return;
    }
    error!("Failed to initialize OpenID");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_openid() {
        init_once().await;
        let openid = OPENID_W3.get().unwrap();
        let (u, c, n) = openid.get_client_resources();
        println!("{u:?} {c:?} {n:?}");
    }
}
