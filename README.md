# nixpkgs-review-gha

Exact-revision GitHub repository reviews through Nix. This retains the existing
fork of [Defelo/nixpkgs-review-gha](https://github.com/Defelo/nixpkgs-review-gha),
its history, MIT attribution, multi-platform mechanics, and Attic/Cachix setting
names. [Simit](https://github.com/caniko/simit/pull/31) now owns the Rust engine,
versioned contracts, adapter, and controller/client generation. This fork is an
exact pinned controller deployment, owning policy, credentials, compatibility
entry points, and activation. `repo-review` remains the compatibility frontend;
`simit review` exposes the same engine. Git, Nix, GitHub CLI,
[nixpkgs-review](https://github.com/Mic92/nixpkgs-review), Attic and Cachix remain
the protocol/build clients, pinned through `flake.lock`.

**Activation status:** implementation requires a reviewed default-branch merge
before `review-repository.yml` can be dispatched normally. A PR or YAML file is
not evidence of a running service. See [activation](docs/activation.md),
[inspection](docs/inspection.md), [verification](docs/verification.md), and
generated `review-service.json`.

See [Simit controller ownership and updates](docs/simit-controller.md) for the
engine/controller boundary and exact-pin migration workflow.

## Backends

| Backend | Selection |
|---|---|
| `nixpkgs` | NixOS/nixpkgs PR changes, using the pinned nixpkgs-review change detector |
| `flake` | Explicit `packages.SYSTEM.NAME` and `checks.SYSTEM.NAME` from the exact target revision |
| `external-flake` | Explicit outputs from a separately pinned, policy-approved recipe; one direct `flake = false` input receives the exact source |

Only GitHub coordinates are supported. No flake and no recipe means
**unsupported**. There are no per-project engine conditionals, inferred build
commands, arbitrary hooks, arbitrary Nix config, or arbitrary runner labels.
Dynamic/content-addressed derivations without known output paths are explicitly
unsupported by the initial frozen-path contract.

The Nixpkgs adapter pins the 3.7.0 Python API and rejects version drift. It uses
upstream `Review.build_commit` and `nix_eval`, replacing only the realization
boundary to freeze selections first. It fetches exact commits, not `pr N` again.
Head mode does not require mergeability. Merge mode checks exact `[base, head]`
parents. Empty successful selection is `no_changes`; missing reports are failure.
The small recorded fixture is **not** a live Nixpkgs review pass.

## CLI and request

```sh
nix run github:OWNER/nixpkgs-review-gha/CONTROLLER_SHA#repo-review -- schema request
nix run github:OWNER/nixpkgs-review-gha/CONTROLLER_SHA#repo-review -- example
repo-review validate request.json
repo-review plan request.json --controller OWNER/nixpkgs-review-gha \
  --revision CONTROLLER_SHA --root TRUSTED-CONTROLLER --output plan.json
repo-review dispatch request.json --controller OWNER/nixpkgs-review-gha \
  --revision CONTROLLER_SHA --dispatch-ref CONTROLLER_BRANCH_OR_TAG
repo-review status --controller OWNER/nixpkgs-review-gha --run RUN_ID --wait-seconds 120
repo-review report --controller OWNER/nixpkgs-review-gha --run RUN_ID --attempt 1 --output new-report-dir
```

File input and `-` (stdin) use the same strict versioned request. Unknown fields,
conflicting selectors, zero targets, unsafe names/paths, token-bearing URLs,
unknown profiles and malformed coordinates are rejected. JSON Schemas and
examples are generated from the Rust types, with cross-field semantic validation
in `repo-review validate`. Never substitute fake 40-character SHAs for unresolved
revisions. The [OpenPencil template](examples/openpencil-external.template.json)
is deliberately invalid until Prompt 02 supplies real source and recipe commits.

Commands emit JSON. Exit codes: `0` completed/passed (a status query may describe
a still-running run), `2` invalid/blocked/tool error, `3` failed build/aggregate,
`4` unsupported backend/target. A closure-export failure returns `3` while retaining
the actual build/test outcomes. Status polling is bounded to 300 seconds. Dispatch
prints the request identity and run-list command; use the returned GitHub run ID
and attempt for later invocations. There is no background daemon.

## Workflow use

Dispatch `.github/workflows/review-repository.yml` with one string input,
`request`. Reusable callers must pin a **full reviewed controller SHA**:

The legacy `build.yml` wrapper supports `publication: none` only. Submit
`publication: request-approval` directly through `review-repository.yml` so
promotion can authenticate the reviewed source workflow path.

```yaml
permissions:
  contents: read
  id-token: write
jobs:
  review:
    uses: OWNER/nixpkgs-review-gha/.github/workflows/review-repository.yml@FULL_REVIEWED_SHA
    with:
      request: '{"schema_version":1,"repository":"OWNER/PROJECT","revision":"EXACT_SHA","backend":"flake","systems":["x86_64-linux"],"packages":["default"],"checks":["smoke"]}'
```

The bootstrap job obtains GitHub's authenticated OIDC identity and uses
`job_workflow_ref` / `job_workflow_sha` to check out the **called controller**.
There is no caller-controlled controller override. Only this bootstrap job has
OIDC permission; build jobs have `contents: read` and no publication secrets.
External reusable calls retain reports in caller artifacts; report credentials
and publication authority are never inherited from callers.

Runner profile `hosted-v1`: `ubuntu-24.04` → x86_64-linux,
`ubuntu-24.04-arm` → aarch64-linux, `macos-15-intel` → x86_64-darwin,
`macos-15` → aarch64-darwin. `uname` and Nix's actual system are checked before
evaluation. No emulation, personal machines or remote builders. Initial live
acceptance is x86_64-linux; other platforms have contract coverage, not an
invented live pass. Darwin uses the explicit `relaxed` sandbox profile; Linux
requires `true`. IFD and flake nixConfig are disabled, jobs/cores are 2, command
build timeout 1800s, silent timeout 600s, whole build job 60 minutes.

`checks-v1` accepts realization (including substitution) without claiming fresh
execution. `checks-rebuild-v1` explicitly rebuilds each selected check; unrelated
dependencies are not rebuilt. Every selected derivation's complete outputs and
runtime closure are exported, bounded to 4 GiB of artifact data / 8 GiB NAR data.

## Publication and consumption

Publication defaults to `none`. `request-approval` records a pending gate; it
does not grant cache access. An authorized operator dispatches
`publish-review.yml` on the default branch with the exact source run, attempt,
controller SHA, system, effective-plan digest and bundle digest. The service
verifies successful source-run and per-platform job conclusions for the exact
attempt, complete collected platform results, workflow identity, authorized
actors, reviewed ancestry,
schemas, file hashes, source identities, systems, selected paths and full closure.
A separate fresh publisher imports only approved data; another fresh-store job
uses **only exact-path `nix copy --from`**, signature checking and closure identity
comparison. No builder fallback is possible through that command path.

Trusted cache profiles are configured in reviewed `policy.json`:

- `attic-existing` reuses `ATTIC_SERVER`, `ATTIC_CACHE`, `ATTIC_TOKEN`.
- `cachix-existing` reuses `CACHIX_CACHE`, `CACHIX_AUTH_TOKEN`, optional
  `CACHIX_SIGNING_KEY`.

No URL/key is invented. The initial policy has no verified public keys and leaves
publication **disabled**. Populate a profile from verified existing cache
configuration and review it before enabling publication. Prefer a quarantine
cache; signing an untrusted build is not a claim that its code is trustworthy.
See [security and profile configuration](docs/security.md).

The artifact contains `review-result.json`, `report.md`, `consume.md`, per-system
effective locks/plans, NAR cache data, file digests and logs. Build, tests, closure export,
publication and retrieval have separate outcomes. Cache publication/retrieval
receipts are separate artifacts linked by the exact effective-plan and bundle
digests; the original immutable build report is not rewritten after promotion.

`repo-review fetch` reads a trusted local cache profile, validates the manifest,
and copies exact paths into a **new** isolated store. It never evaluates a flake
or builds. Signature checking is explicitly enabled even when the consumer's
configuration has `require-sigs = false`. Standard cache keys are retained and
approved keys appended. New keys
grant trust to the cache operator to provide executable software. Unsigned local
transfers are permitted only by `verify-local` into a disposable isolated store,
or inside the fresh publisher after digest-bound approval; user fetch never uses
`--no-check-sigs`. Fetch does not modify or activate NixOS/Home Manager.

## Comments and browser shortcut

Posting defaults off. `GH_TOKEN` is read only by the isolated report job; configure
a narrowly scoped App installation token or fine-grained token authorized on the
target repository. The workflow repository's `GITHUB_TOKEN` is not cross-repo
authority. Without a report credential, the result says `posted: false` and emits
the manual `gh pr comment --body-file` argv. Comments include the exact commit and
run, are bounded, and are refused if head/base moved. No auto-approval or merge.

Install `shortcut.user.js` and explicitly set
`localStorage.setItem("repo-review-controller", "OWNER/CONTROLLER-REPOSITORY")`
on GitHub. The shortcut supports arbitrary GitHub PR coordinates and prepares the
same editable request. It neither assumes the controller's name nor dispatches
without user interaction.

## Legacy migration

`review.yml` retains PR/platform inputs through the shared Rust adapter, with
explicit merge mode. Defaults now select x86_64-linux only, no posting and no
publication. `extra-args`, `upterm`, `push-to-cache=true`, non-`nothing` success
actions and custom sandbox settings are rejected. `build.yml` now accepts the
structured request rather than interpolating package strings. `EXTRA_NIX_CONFIG`,
`BUILDERS`, `USE_BUILDERS`, `SSH_KEY`, `SSH_CERT` are not consumed. They are not
deleted from repository settings. Self-update is manual PR-only integration,
never unattended rebase/force-push. Conflicts stop for manual resolution.

## Development

```sh
nix build .#repo-review --out-link tools
tools/bin/simit init ci --review-only --check --diff
nix run .#formatter.x86_64-linux -- --ci
```

Engine Rust tests,
Clippy, Rust 1.85 and Windows compatibility checks live in Simit. Run controller
Nix builds and closure checks in CI under the operator's CI-only policy.
`simit.toml` declares the controller role. Generated review/publication workflows
and the setup action come from the exact locked Simit source; policy and legacy
workflows remain deployment-owned. `verify-engine` rejects a running engine/tool
package that differs from the frozen controller lock.

Formatting runs through treefmt only. `check.yml` builds the Nix package (including
Rust tests), checks workflows/schemas, and runs small conventional/external/failing
fixtures plus isolated complete-closure transfer. The pinned Python check exercises
real upstream change detection with recorded head/merge/no-change inputs. CI also
runs Simit's otherwise ignored `tests/review_retrieval.rs` tests from the exact
engine revision against the exported closure:
actual cache miss, unsigned rejection, wrong-key rejection, and signed exact-copy
success, including a consumer configured with `require-sigs = false`. No target
cache credentials are
used. No live upstream review is fabricated by fixture tests.
