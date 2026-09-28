# SDK conformance suite (task 5.1)

Golden host-call transcripts and payloads per plugin capability, produced by
the Go SDK and replayed against the Rust SDK. A capability added to one SDK
without the other fails loudly instead of drifting silently.

## Layout

- `testdata/manifest.json` — the capability registry (11 entries). Both sides
  iterate it.
- `testdata/<capability>.json` — one golden transcript per capability.
- `../src/conformance.rs` — the Rust replay (unit test
  `replay_conformance_transcripts`, runs in `cargo test`).
- `../src/run_override/tests.rs`, `../src/action/tests.rs` — shared-fixture
  decode coverage (see `../fixtures/README.md`).

## Provenance

Goldens vendored from `serviceradar-sdk-go` commit
`bd15ef53d5bd95b7aea7393c3442bc9776e326a3`
(branch `feat/conformance-transcripts`, produced by
`sdk/conformance_test.go` via `InstallConformanceRecorder`).
Same bytes as `sdk/testdata/conformance/` at that commit.

## Regenerating

Go side (after an intentional SDK change):

```
cd serviceradar-sdk-go
UPDATE_CONFORMANCE=1 go test ./sdk/ -run TestConformanceGoldens
go test ./sdk/ -count=1   # drift check must pass
```

Then vendor the result here (`conformance/testdata/*.json`), run
`cargo test`, and record the new Go commit hash above.

## Transcript schema

`serviceradar-sdk-conformance/v1`: `format`, `capability`, `scenario` (fixed
scenario inputs), `replies` (scripted host-to-plugin bytes), `output`
(derived values for payload goldens that make no host calls), `produced_by`,
`calls` (`func`, `args`, `returns`).

Value encoding, mirrored by both sides:

- UTF-8 byte strings record as JSON strings; other bytes as
  `{"base64": "..."}`.
- Plugin-to-host JSON documents (HTTP request bytes, `submit_result` and
  `emit_telemetry` payloads) record as `{"json": <parsed>}` so key order never
  matters. Host-to-plugin bytes (HTTP responses, read chunks) stay raw: the
  replay feeds them back verbatim, and SDK-level assertions prove they decode.
- Read buffer capacities and wall-clock durations are not recorded.
- Fake handles start at 7 in call order on both sides.
- `rtsp` alone allows `timeout_ms` to differ by up to 100 ms: both RTSP
  clients derive per-call timeouts from remaining deadlines, so the exact
  millisecond is timing-dependent by construction.

## Known non-wire divergence

`plugin_inputs` compares the flattened-item *projection*
(name/entity/query/chunks/item), not the raw `FlattenItems()` JSON: Go
serializes those field names capitalized (Go defaults, no tags) while Rust
uses snake_case. The projection is SDK-local ergonomics, never host ABI, so
the golden pins the logical items and both SDKs keep their casing.
