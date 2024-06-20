#![allow(dead_code)]

use std::{fs::read_to_string, path::PathBuf, str::FromStr, sync::OnceLock};

use openidconnect::{
    core::{self, CoreClient, CoreResponseType},
    reqwest, AuthenticationFlow, Client, ClientId, ClientSecret, CsrfToken, EmptyAdditionalClaims,
    EmptyExtraTokenFields, IdTokenFields, IssuerUrl, Nonce, RevocationErrorResponseType, Scope,
    StandardErrorResponse, StandardTokenIntrospectionResponse, StandardTokenResponse,
};
use url::Url;

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

struct OpenID {
    client: OIDC,
}

static OPENID: OnceLock<OpenID> = OnceLock::new();

impl OpenID {
    fn new() -> Self {
        let table = toml::from_str::<toml::Table>(
            &read_to_string(PathBuf::from_str("config/configurations.toml").unwrap()).unwrap(),
        )
        .unwrap();
        let url = table
            .get("openid")
            .unwrap()
            .get("url")
            .unwrap()
            .as_str()
            .unwrap();
        let client_id = table
            .get("openid")
            .unwrap()
            .get("client")
            .unwrap()
            .get("client_id")
            .unwrap()
            .as_str()
            .unwrap();
        let client_secret = table
            .get("openid")
            .unwrap()
            .get("client")
            .unwrap()
            .get("client_secret")
            .unwrap()
            .as_str()
            .unwrap();

        let provider_metadata = core::CoreProviderMetadata::discover(
            &IssuerUrl::new(url.to_owned()).unwrap(),
            reqwest::http_client,
        )
        .unwrap();

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id.to_string()),
            Some(ClientSecret::new(client_secret.to_string())),
        );

        OpenID { client }
    }

    fn get_client_resources(&self) -> (Url, CsrfToken, Nonce) {
        let (u, c, n) = self
            .client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();
        (u, c, n)
    }
}

pub fn init_once() {
    OPENID.get_or_init(OpenID::new);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openid() {
        init_once();
        let _ = OPENID.get().unwrap();
    }
}
