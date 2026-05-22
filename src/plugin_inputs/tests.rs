use std::collections::BTreeMap;
use std::fs;

use serde_json::json;

use crate::Error;

use super::{PluginInputsPayload, parse_plugin_inputs_json, parse_plugin_inputs_map};

#[test]
fn plugin_inputs_validate_and_flatten() {
    let payload = PluginInputsPayload::parse_json(
        json!({
            "schema": "serviceradar.plugin_inputs.v1",
            "policy_id": "policy-1",
            "policy_version": 1,
            "agent_id": "agent-1",
            "generated_at": "2026-04-03T00:00:00Z",
            "inputs": [{
                "name": "devices",
                "entity": "devices",
                "query": "kind=device",
                "chunk_index": 0,
                "chunk_total": 1,
                "chunk_hash": "abc",
                "items": [
                    {"uid": "dev-1"},
                    {"uid": "dev-2"}
                ]
            }]
        })
        .to_string()
        .as_bytes(),
    )
    .expect("parse plugin inputs");

    assert_eq!(payload.flatten_items().len(), 2);
    assert_eq!(payload.items_by_entity("devices").len(), 2);
    assert_eq!(payload.items_by_name("devices").len(), 2);
}

#[test]
fn parse_plugin_inputs_rejects_invalid_schema() {
    let err = parse_plugin_inputs_json(
        json!({
            "schema": "nope",
            "policy_id": "policy-1",
            "policy_version": 1,
            "agent_id": "agent-1",
            "generated_at": "2026-04-03T00:00:00Z",
            "inputs": [{
                "name": "devices",
                "entity": "devices",
                "query": "kind=device",
                "chunk_index": 0,
                "chunk_total": 1,
                "chunk_hash": "abc",
                "items": [{"uid": "dev-1"}]
            }]
        })
        .to_string()
        .as_bytes(),
    )
    .expect_err("invalid schema should fail");

    assert!(matches!(err, Error::InvalidPluginInputs(_)));
}

#[test]
fn each_item_stops_on_error_and_map_parser_works() {
    let raw = json!({
        "schema": "serviceradar.plugin_inputs.v1",
        "policy_id": "policy-1",
        "policy_version": 1,
        "agent_id": "agent-1",
        "generated_at": "2026-04-03T00:00:00Z",
        "inputs": [{
            "name": "devices",
            "entity": "devices",
            "query": "kind=device",
            "chunk_index": 0,
            "chunk_total": 1,
            "chunk_hash": "abc",
            "items": [{"uid": "dev-1"}, {"uid": "dev-2"}]
        }]
    });

    let map =
        serde_json::from_value::<BTreeMap<String, serde_json::Value>>(raw).expect("map payload");
    let payload = parse_plugin_inputs_map(&map).expect("parse inputs map");

    let mut seen = 0;
    let err = payload.each_item(|_| {
        seen += 1;
        if seen == 2 {
            return Err(Error::Message("stop".to_string()));
        }
        Ok(())
    });

    assert!(matches!(err, Err(Error::Message(_))));
    assert_eq!(seen, 2);
}

#[test]
fn plugin_inputs_support_try_from_and_iteration() {
    let raw = json!({
        "schema": "serviceradar.plugin_inputs.v1",
        "policy_id": "policy-1",
        "policy_version": 1,
        "agent_id": "agent-1",
        "generated_at": "2026-04-03T00:00:00Z",
        "inputs": [{
            "name": "devices",
            "entity": "devices",
            "query": "kind=device",
            "chunk_index": 0,
            "chunk_total": 1,
            "chunk_hash": "abc",
            "items": [{"uid": "dev-1"}, {"uid": "dev-2"}]
        }]
    });

    let payload =
        PluginInputsPayload::try_from(raw.to_string().as_bytes()).expect("try_from bytes");
    let items: Vec<_> = payload.iter_items().collect();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "devices");

    let from_iter: Vec<_> = (&payload).into_iter().collect();
    assert_eq!(from_iter.len(), 2);

    let map =
        serde_json::from_value::<BTreeMap<String, serde_json::Value>>(raw).expect("map payload");
    let from_map = PluginInputsPayload::try_from(&map).expect("try_from map");
    assert_eq!(from_map.flatten_items().len(), 2);
}

#[test]
fn plugin_inputs_decode_target_contexts() {
    let payload = PluginInputsPayload::parse_json(
        json!({
            "schema": "serviceradar.plugin_inputs.v1",
            "policy_id": "monitoring_binding:binding-1",
            "policy_version": 1,
            "agent_id": "agent-a",
            "generated_at": "2026-05-21T23:00:00Z",
            "inputs": [{
                "name": "monitoring_binding:binding-1",
                "entity": "monitoring_checks",
                "query": "monitoring_binding_id=binding-1",
                "chunk_index": 0,
                "chunk_total": 1,
                "chunk_hash": "abc",
                "items": [{
                    "uid": "check-1",
                    "check_instance_id": "check-1",
                    "monitoring_binding_id": "binding-1",
                    "descriptor_id": "http.url.availability",
                    "descriptor_version": "1.0.0",
                    "target_kind": "service",
                    "target": {
                        "monitored_service_id": "service-1",
                        "endpoint_url": "https://example.test/health",
                        "host": "example.test",
                        "port": 443,
                        "path": "/health",
                        "device_uid": "sr:device-1"
                    },
                    "credential_policy": {
                        "credential_brokers": [{
                            "grant_id": "grant-1",
                            "credential_secret_ref": "credentialref:network-credential-secret:secret-1",
                            "grant_type": "http_auth"
                        }]
                    }
                }]
            }]
        })
        .to_string()
        .as_bytes(),
    )
    .expect("parse target payload");

    let contexts = payload.target_contexts().expect("target contexts");
    assert_eq!(contexts.len(), 1);

    let ctx = &contexts[0];
    assert_eq!(ctx.check_instance_id, "check-1");
    assert_eq!(ctx.monitored_service_id(), Some("service-1"));
    assert_eq!(ctx.device_uid(), Some("sr:device-1"));
    assert_eq!(ctx.endpoint_url(), Some("https://example.test/health"));
    assert_eq!(ctx.port(), Some(443));
    assert_eq!(ctx.credential_grants().len(), 1);
}

#[test]
fn service_monitoring_input_fixture_decodes() {
    let raw = fs::read("testdata/service_monitoring_input.json").expect("fixture");
    let payload = parse_plugin_inputs_json(&raw).expect("parse fixture");
    let contexts = payload.target_contexts().expect("target contexts");

    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].check_instance_id, "check-1");
}
