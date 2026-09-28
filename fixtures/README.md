# Shared SDK fixtures

These JSON files are stable test fixtures for SDK encode/decode coverage.
They are byte-identical with `serviceradar-sdk-go/fixtures/` — that is the
rule, not a coincidence: extend a fixture in both SDKs at once, and verify
with `sha256sum` (or `cmp`) before committing.

Shared files:

- `northbound_action_descriptor.json`
- `northbound_action_invocation.json`
- `northbound_action_result.json`
- `northbound_action_result_run_overrides.json`
- `northbound_action_deferred_result.json`
- `northbound_action_poll_request.json`
- `northbound_action_polling_result.json`
- `plugin_run_overrides_config.json`

They are not runtime defaults and operators are not expected to edit them.
Synthetic values only: private (RFC 1918) or documentation (`192.0.2.0/24`,
`example.com` hosts) addresses, invented serials and job IDs.
