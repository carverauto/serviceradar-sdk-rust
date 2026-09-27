<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->
## Pipeline test agents (no-mistakes gate)

The gate's test step has a 10-minute budget and starts in a fresh worktree, so a
cold `target/` directory can use the whole budget compiling. When you are the
gate's test agent:

- Build into the shared target directory so compiled dependencies survive
  between runs: `export CARGO_TARGET_DIR="$HOME/.cache/serviceradar-sdk-rust/target"`.
- Run `cargo test --all-targets` and `cargo clippy --all-targets --locked -- -D warnings`
  for the change. Add focused tests for the behaviour the branch changes.
- Leave `cargo build --examples --target wasm32-unknown-unknown` and
  `cargo publish --dry-run` to GitHub CI (`.github/workflows/ci.yml` runs both on
  every pull request) unless the branch changes an example or the packaging.
