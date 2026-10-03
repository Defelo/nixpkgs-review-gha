# Pinned Simit controller deployment

The implementation is consolidated in [Simit PR #31](https://github.com/caniko/simit/pull/31).
This repository remains the controller deployment and preserves its fork history,
upstream attribution, legacy entry points, and cache variable names.

## Ownership

| Simit | Controller deployment |
| --- | --- |
| Rust engine and `simit review` / `repo-review` frontends | Exact Simit and Nixpkgs source pins in `flake.lock` |
| Versioned contracts, Python selection adapter and tool constructor | Reviewed recipe and public-cache policy in `policy.json` |
| Controller/client workflow templates and drift checking | Existing secret names, protected publication environment and approval |
| Rust/MSRV/platform regression tests | Legacy entry points, acceptance fixtures, service descriptor and activation |

`inputs.simit.flake = false` keeps the deployment independent of Simit's website
and development dependency graph. `nix/review-tools.nix` comes from that exact
locked source and packages both frontends, the adapter and tools. The package's
immutable engine manifest must match the controller lock before plan resolution,
build, collection, reporting or promotion.

The review workflows and `.github/actions/setup-nix/action.yml` are generated
through `[review] role = "controller"` in `simit.toml`. After building the pinned
package in an approved environment, regenerate or check them with:

```sh
tools/bin/simit init ci --review-only
tools/bin/simit init ci --review-only --check --diff
tools/bin/repo-review verify-engine --root .
tools/bin/repo-review engine-info
```

Policy is never generated from ordinary Simit CI or release Attic settings.
`ATTIC_SERVER`, `ATTIC_CACHE` and `ATTIC_TOKEN` retain their existing names.
Cache profiles remain empty and publication remains disabled. The external
fixture recipe is explicitly pinned to the preserved pre-migration fixture
commit, rather than implicitly trusting any controller fixture directory.

## Updating the engine

Review the exact Simit candidate and its CI first. Update only the engine source
pin and obtain a Nix-generated lock receipt in CI, preserving the existing
Nixpkgs tool lock unless a tool update is separately intended. Regenerate managed
files with that candidate's generator. Check schemas and conventional, external,
failing-check, complete-export, cache-miss and signature gates. Commit the exact
pins and generated files for review; updates do not activate publication.

The engine tests formerly in this repository now live in Simit. Controller CI
checks out the exact manifest revision for the real-Nix retrieval tests; it never
uses a moving branch or installs a floating engine dependency. The v1 plan keeps
the complete tool lock, including the engine source identity, and result/bundle
digests continue to bind promotion approvals.

## Activation

Review both draft PRs and merge manually, engine first and controller second.
Follow [activation.md](activation.md) after default-branch workflow registration.
The migration and green fixture checks do not establish live readiness.
Publication, public-key trust and OpenPencil onboarding require their separate
recorded acceptance gates; `ready: false` remains deliberate.
