use std::collections::BTreeMap;

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
