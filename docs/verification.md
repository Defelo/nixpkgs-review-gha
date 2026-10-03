# Verification and readiness

Infrastructure PR: <https://github.com/caniko/nixpkgs-review-gha/pull/1>

## Review corrections

| Finding | Correction | Regression coverage |
|---|---|---|
| Packaged Nixpkgs adapter could not import its application | Convert the pinned Python application to a module for the selector environment | Packaged import check and real upstream `Review.build_commit`/`differences` using recorded head, merge, and no-change inputs |
| Pinned Attic does not support `--config` | Private `$XDG_CONFIG_HOME/attic/config.toml`; explicit `review:CACHE` destination | Configuration permissions, argv/environment checks, and pinned `attic push --help` |
| Blocked selection could pass requested tests; failed export could exit zero | Explicit closure-export outcome and pipeline completion predicate | Blocked selection, fake-tool export failure through the CLI, retained build/test facts, and nonzero exit |
| Promotion accepted completed failed/cancelled runs and partial platform success | Require successful exact-attempt origin jobs and a complete validated collected review | Failed, cancelled, skipped, wrong-attempt, wrong-platform, duplicate/missing jobs, and tampered aggregates/digests |
| Consumer `require-sigs = false` could disable verification | Enable signatures explicitly on the command and destination store; append reviewed keys | Argument regression plus real unsigned/wrong-key rejection and signed exact-copy success under an unsafe consumer configuration |
| Recorded report helper and cache-miss fixture did not exercise production boundaries | Replace unused report validator with actual selection/bundle coverage; validate the bundle before retrieving from a separate empty cache | Missing/tampered no-change selection evidence and actual Nix copy failure |

## Evidence

- Local `cargo test --locked`: 19 passing contract/CLI tests. The two real-Nix
  retrieval tests are explicitly CI-only and excluded from the ordinary local run.
- Local `cargo clippy --locked --all-targets -- -D warnings`: passed.
- Local treefmt, actionlint 1.7.12, Python syntax, and Git whitespace checks: passed.
- [Initial push CI](https://github.com/caniko/nixpkgs-review-gha/actions/runs/37110054510)
  and [initial PR CI](https://github.com/caniko/nixpkgs-review-gha/actions/runs/37110087174)
  passed for `6f37ffdf911f873d9b03db0ee337449a4bb820c1`.
- [Expanded verification CI](https://github.com/caniko/nixpkgs-review-gha/actions/runs/37110648678)
  passed for `0702bbb1319d68cf0fea88df1207606e336eb9d4`, including packaged
  selection, conventional/external/failing fixtures, complete closure transfer,
  and both real retrieval tests (`2 passed; 0 ignored`).

Nix evaluation, packaging, builds, and closure checks run in GitHub CI only, as
requested. Fixture signature keys are ephemeral and used only for offline
file-cache checks; they are unrelated to the existing production cache policy.

## Readiness boundary

The implementation is published for review. Live service acceptance requires
human review and a manual default-branch merge, followed by the exact commands in
[activation.md](activation.md). No live workflow dispatch, real Nixpkgs PR pass,
production cache publication/retrieval, or host activation is claimed here.

`policy.json` has no configured cache profiles and keeps publication disabled.
`review-service.json` remains `ready: false`. The next OpenPencil task can use the
generic external-flake request/schema after live acceptance; its source/recipe
commits and recipe-policy approval are still unresolved.
