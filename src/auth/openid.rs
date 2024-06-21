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

use crate::errors::{GuardianError, Result};

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

pub struct OpenID {
    client: OIDC,
}

pub static OPENID: OnceLock<OpenID> = OnceLock::new();

impl OpenID {
    async fn new() -> Result<Self> {
        let table = toml::from_str::<toml::Table>(&read_to_string(PathBuf::from_str(
            "config/configurations.toml",
        )?)?)?;
        let openid_table = table
            .get("openid")
            .ok_or_else(|| GuardianError::TomlLookupError)?;

        let url = openid_table
            .get("url")
            .ok_or_else(|| GuardianError::TomlLookupError)?
            .as_str()
            .ok_or_else(|| GuardianError::StringConversionError)?;
        let redirect = openid_table
            .get("redirect_url")
            .ok_or_else(|| GuardianError::TomlLookupError)?
            .as_str()
            .ok_or_else(|| GuardianError::StringConversionError)?;

        let client = openid_table
            .get("client")
            .ok_or_else(|| GuardianError::TomlLookupError)?;

        let client_id = client
            .get("client_id")
            .ok_or_else(|| GuardianError::TomlLookupError)?
            .as_str()
            .ok_or_else(|| GuardianError::StringConversionError)?;
        let client_secret = client
            .get("client_secret")
            .ok_or_else(|| GuardianError::TomlLookupError)?
            .as_str()
            .ok_or_else(|| GuardianError::StringConversionError)?;

        let provider_metadata = core::CoreProviderMetadata::discover_async(
            IssuerUrl::new(url.to_owned()).unwrap(),
            reqwest::async_http_client,
        )
        .await
        .map_err(|e| GuardianError::GeneralError(e.to_string()))?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id.to_string()),
            Some(ClientSecret::new(client_secret.to_string())),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect.to_string())
                .map_err(|e| GuardianError::GeneralError(e.to_string()))?,
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
            .map_err(|e| GuardianError::TokenRequestError(e.to_string()))?;
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
    if let Ok(openid) = OpenID::new().await {
        OPENID.get_or_init(|| openid);
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
        let openid = OPENID.get().unwrap();
        let (u, c, n) = openid.get_client_resources();
        println!("{:?} {:?} {:?}", u, c, n);
    }
}
