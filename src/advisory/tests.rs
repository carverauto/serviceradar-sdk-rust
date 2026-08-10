use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{
    ADVISORY_FEED_CONTRACT_VERSION, AdvisoryFeedBatch, AdvisoryRecord, AdvisorySnapshot,
    AdvisorySource, AffectedCoordinate, PluginResult,
};

#[test]
fn advisory_feed_batch_serializes_generic_contract() {
    let mut range = BTreeMap::new();
    range.insert("fixed_version".to_string(), json!("3.0.14"));

    let batch = AdvisoryFeedBatch::new(
        "com.example.feed",
        AdvisorySource::new("example", "normalized").with_display_name("Example Normalized Feed"),
        AdvisorySnapshot::new("vulnerability-feeds/example/sha256.json", "a".repeat(64))
            .with_content_type("application/json")
            .with_size_bytes(42),
    )
    .with_advisory(
        AdvisoryRecord::new("CVE-2026-1")
            .with_cve("CVE-2026-1")
            .with_severity("high")
            .with_coordinate(
                AffectedCoordinate::purl("pkg:deb/debian/openssl@3.0.13?arch=amd64")
                    .with_match_semantics("producer_normalized")
                    .with_version_range(range),
            )
            .with_reference("https://example.test/CVE-2026-1"),
    );

    let payload = serde_json::to_value(&batch).expect("serialize advisory batch");
    assert_eq!(payload["schema_version"], ADVISORY_FEED_CONTRACT_VERSION);
    assert_eq!(payload["source"]["provider"], "example");
    assert_eq!(
        payload["advisories"][0]["affected_coordinates"][0]["type"],
        "purl"
    );

    let encoded = payload.to_string().to_lowercase();
    assert!(!encoded.contains("vulncheck"));
    assert!(!encoded.contains("cisa"));
    assert!(!encoded.contains("nvd"));
}

#[test]
fn plugin_result_serializes_advisory_feed_batch() {
    let batch =
        AdvisoryFeedBatch::new(
            "com.example.feed",
            AdvisorySource::new("example", "normalized"),
            AdvisorySnapshot::new("vulnerability-feeds/example/sha256.json", "b".repeat(64)),
        )
        .with_advisory(AdvisoryRecord::new("CVE-2026-2").with_coordinate(
            AffectedCoordinate::cpe("cpe:2.3:a:example:package:1.0:*:*:*:*:*:*:*"),
        ));

    let payload = PluginResult::ok("submitted advisory batch")
        .with_advisory_feed(batch)
        .serialize()
        .expect("serialize plugin result");
    let decoded: Value = serde_json::from_slice(&payload).expect("decode plugin result");
    assert_eq!(
        decoded["advisory_feeds"][0]["schema_version"],
        ADVISORY_FEED_CONTRACT_VERSION
    );
}
