use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{
    CREDENTIAL_INJECT_BEARER_TOKEN, CREDENTIAL_INJECT_KEY_ALLOW_INSECURE_TLS,
    CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS, OAuth2ClientCredentialsInject,
    oauth2_client_credentials_form_fields,
};
use crate::error::Error;
use crate::plugin_inputs::CredentialBrokerGrant;

pub(crate) const GRANT_FIXTURE: &str =
    include_str!("../../fixtures/credential_grant_oauth2_client_credentials.json");

pub(crate) fn fixture_oauth2_grant() -> CredentialBrokerGrant {
    serde_json::from_str(GRANT_FIXTURE).expect("decode grant fixture")
}

#[test]
fn oauth2_client_credentials_inject_matches_shared_fixture() {
    let grant = fixture_oauth2_grant();

    let built = OAuth2ClientCredentialsInject::new("auth.example.com", 443, "/oauth2/token")
        .with_scope("devices.read")
        .inject()
        .unwrap();
    assert_eq!(built, grant.inject);
    assert_eq!(
        grant.inject_type(),
        CREDENTIAL_INJECT_OAUTH2_CLIENT_CREDENTIALS
    );
    let allow = grant.allow.as_ref().expect("allow scope");
    assert_eq!(allow.hosts, vec!["api.example.com"]);
    assert_eq!(allow.ports, vec![443]);
    assert_eq!(allow.methods, vec!["GET"]);
    assert_eq!(allow.paths, vec!["/v1/*"]);
    oauth2_client_credentials_form_fields(&grant.inject_spec()).expect("fixture spec is valid");
}

#[test]
fn credential_grant_fixture_round_trips() {
    let grant = fixture_oauth2_grant();
    assert_eq!(
        serde_json::to_value(&grant).unwrap(),
        serde_json::from_str::<Value>(GRANT_FIXTURE).unwrap()
    );
}

#[test]
fn oauth2_client_credentials_inject_optional_keys() {
    let duplicate = OAuth2ClientCredentialsInject::new("auth.example.com", 8443, "/token")
        .with_field("app_id", "client_id")
        .with_request_target("get", "api.example.com", "/v1/devices")
        .spec();
    assert!(duplicate.is_err(), "duplicate client_id mapping must fail");

    let spec = OAuth2ClientCredentialsInject::new("auth.example.com", 8443, "/token")
        .with_request_target("get", "api.example.com", "/v1/devices")
        .spec()
        .unwrap();
    for (key, want) in [
        ("token_port", "8443"),
        ("method", "GET"),
        ("host", "api.example.com"),
        ("path", "/v1/devices"),
    ] {
        assert_eq!(spec.get(key).map(String::as_str), Some(want), "{key}");
    }
    assert!(!spec.contains_key(CREDENTIAL_INJECT_KEY_ALLOW_INSECURE_TLS));

    let insecure = OAuth2ClientCredentialsInject::new("auth.example.com", 443, "/token")
        .with_allow_insecure_tls(true)
        .spec()
        .unwrap();
    assert_eq!(
        insecure
            .get(CREDENTIAL_INJECT_KEY_ALLOW_INSECURE_TLS)
            .map(String::as_str),
        Some("true")
    );
}

#[test]
fn oauth2_client_credentials_inject_validation() {
    let base = OAuth2ClientCredentialsInject::new("auth.example.com", 443, "/oauth2/token");
    let cases = [
        (
            "method",
            OAuth2ClientCredentialsInject {
                token_method: "GET".into(),
                ..base.clone()
            },
        ),
        (
            "host url",
            OAuth2ClientCredentialsInject {
                token_host: "https://auth.example.com".into(),
                ..base.clone()
            },
        ),
        (
            "port",
            OAuth2ClientCredentialsInject {
                token_port: 0,
                ..base.clone()
            },
        ),
        (
            "relative path",
            OAuth2ClientCredentialsInject {
                token_path: "oauth2/token".into(),
                ..base.clone()
            },
        ),
        (
            "path query",
            OAuth2ClientCredentialsInject {
                token_path: "/token?x=1".into(),
                ..base.clone()
            },
        ),
        (
            "grant type",
            base.clone().with_fixed("grant_type", "password"),
        ),
        (
            "field key",
            base.clone().with_field("Client Secret", "client_secret_2"),
        ),
        (
            "no secret",
            OAuth2ClientCredentialsInject {
                fields: BTreeMap::from([("client_id".to_string(), "client_id".to_string())]),
                ..base.clone()
            },
        ),
    ];
    for (name, spec) in cases {
        match spec.validate() {
            Err(Error::InvalidCredentialInject(message)) => assert!(
                message.starts_with("invalid oauth2_client_credentials inject spec: "),
                "{name}: {message}"
            ),
            other => panic!("{name}: expected invalid spec, got {other:?}"),
        }
    }
    base.validate().expect("default spec is valid");
}

#[test]
fn credential_broker_grant_inject_spec_stringifies() {
    let grant = CredentialBrokerGrant {
        inject: BTreeMap::from([
            ("type".to_string(), json!(" Bearer_Token ")),
            ("token_port".to_string(), json!(443)),
            ("allow_insecure_tls".to_string(), json!(true)),
            ("empty".to_string(), Value::Null),
        ]),
        ..CredentialBrokerGrant::default()
    };
    assert_eq!(grant.inject_type(), CREDENTIAL_INJECT_BEARER_TOKEN);
    let spec = grant.inject_spec();
    assert_eq!(spec.get("token_port").map(String::as_str), Some("443"));
    assert_eq!(
        spec.get("allow_insecure_tls").map(String::as_str),
        Some("true")
    );
    assert!(!spec.contains_key("empty"));
    assert_eq!(CredentialBrokerGrant::default().inject_type(), "");
}
