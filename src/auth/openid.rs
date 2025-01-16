#![allow(dead_code)]

use std::{fs::read_to_string, path::PathBuf, str::FromStr, sync::OnceLock};

use openidconnect::{
    core::{self, CoreClient, CoreResponseType},
    reqwest, AuthenticationFlow, AuthorizationCode, Client, ClientId, ClientSecret, CsrfToken,
    EmptyAdditionalClaims, EmptyExtraTokenFields, IdTokenFields, IssuerUrl, Nonce, RedirectUrl,
    RevocationErrorResponseType, Scope, StandardErrorResponse, StandardTokenIntrospectionResponse,
    StandardTokenResponse,
};
use tracing::error;
use url::Url;

use crate::errors::{BridgeError, Result};

#[allow(clippy::upper_case_acronyms)]
// LMFAO
type OIDC = Client<
    EmptyAdditionalClaims,
    core::CoreAuthDisplay,
    core::CoreGenderClaim,
    core::CoreJweContentEncryptionAlgorithm,
    core::CoreJwsSigningAlgorithm,
    core::CoreJsonWebKeyType,
    core::CoreJsonWebKeyUse,
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
            core::CoreJsonWebKeyType,
        >,
        core::CoreTokenType,
    >,
    core::CoreTokenType,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, core::CoreTokenType>,
    core::CoreRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
>;

type Token = StandardTokenResponse<
    IdTokenFields<
        EmptyAdditionalClaims,
        EmptyExtraTokenFields,
        core::CoreGenderClaim,
        core::CoreJweContentEncryptionAlgorithm,
        core::CoreJwsSigningAlgorithm,
        core::CoreJsonWebKeyType,
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
}

pub static OPENID_W3: OnceLock<OpenID> = OnceLock::new();
pub static OPENID_IBM: OnceLock<OpenID> = OnceLock::new();

impl OpenID {
    async fn new(table_name: OpenIDProvider) -> Result<Self> {
        let table = toml::from_str::<toml::Table>(&read_to_string(PathBuf::from_str(
            "config/configurations.toml",
        )?)?)?;
        let openid_table = table
            .get(table_name.into())
            .ok_or_else(|| BridgeError::TomlLookupError)?;

        let url = openid_table
            .get("url")
            .ok_or_else(|| BridgeError::TomlLookupError)?
            .as_str()
            .ok_or_else(|| BridgeError::StringConversionError)?;
        let redirect = openid_table
            .get("redirect_url")
            .ok_or_else(|| BridgeError::TomlLookupError)?
            .as_str()
            .ok_or_else(|| BridgeError::StringConversionError)?;

        let client = openid_table
            .get("client")
            .ok_or_else(|| BridgeError::TomlLookupError)?;

        let client_id = client
            .get("client_id")
            .ok_or_else(|| BridgeError::TomlLookupError)?
            .as_str()
            .ok_or_else(|| BridgeError::StringConversionError)?;
        let client_secret = client
            .get("client_secret")
            .ok_or_else(|| BridgeError::TomlLookupError)?
            .as_str()
            .ok_or_else(|| BridgeError::StringConversionError)?;

        let provider_metadata = core::CoreProviderMetadata::discover_async(
            IssuerUrl::new(url.to_owned())?,
            reqwest::async_http_client,
        )
        .await
        .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id.to_string()),
            Some(ClientSecret::new(client_secret.to_string())),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect.to_string())
                .map_err(|e| BridgeError::GeneralError(e.to_string()))?,
        );

        Ok(OpenID { client })
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
            .exchange_code(AuthorizationCode::new(code))
            .request_async(reqwest::async_http_client)
            .await
            .map_err(|e| BridgeError::TokenRequestError(e.to_string()))?;
        Ok(token)
    }

    pub fn get_verifier(
        &self,
    ) -> openidconnect::IdTokenVerifier<
        core::CoreJwsSigningAlgorithm,
        core::CoreJsonWebKeyType,
        core::CoreJsonWebKeyUse,
        core::CoreJsonWebKey,
    > {
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
        println!("{:?} {:?} {:?}", u, c, n);
    }
}
