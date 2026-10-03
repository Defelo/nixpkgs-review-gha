# Trust boundaries and profiles

1. **Resolve:** bootstrap identifies the controller from GitHub-issued workflow
   identity, checks out that exact commit, builds only trusted controller tools,
   and freezes GitHub repository IDs/SHAs/trees/PR parents and the tool lock.
   No target Nix locking or evaluation occurs here.
2. **Build:** fresh disposable hosted machines fetch and verify exact Git trees,
   evaluate untrusted Nix, freeze per-system locks/derivations/output paths, then
   realize those exact derivations. IFD/nixConfig are disabled. Outputs and logs
   remain untrusted observations. No signing, cache, production SSH or write
   credentials are present. A Nix sandbox is not a whole-runner security boundary.
3. **Collect:** fresh trusted controller validates every platform against the
   resolver's job-output digest and validates effective-plan/file/closure digests.
   Missing or cancelled platforms cannot be success. It generates Markdown itself,
   never runs generated scripts and never copies untrusted Markdown into comments.
4. **Publish:** explicit default-branch dispatch by a write/maintain/admin actor
   approves exact effective-plan and bundle digests, source run and attempt. The
   source must be the known workflow path, dispatched by an authorized actor,
   and its controller commit must be an ancestor of reviewed default-branch code.
   A fresh publisher checks these identities again and loads current trusted cache
   policy. Environment `review-publication` can add an administrator-managed second
   review; it is not silently provisioned or substituted for digest approval.
5. **Verify:** another job retrieves into a new isolated store using only exact
   paths, validates signatures with approved keys and compares the complete NAR
   hash/reference graph. It never evaluates/builds a target or falls back on a miss.
6. **Report:** isolated credentials limited to comments on the target repository;
   no auto-approval/merge. External reusable calls retain artifacts for manual
   posting rather than obtaining controller secrets.

## Cache profile configuration

`policy.json` is administrator-owned controller source. A cache profile has:

```json
{
  "kind": "attic",
  "server": "VERIFIED_EXISTING_ATTIC_SERVER",
  "cache": "EXISTING_ATTIC_CACHE_NAME",
  "url": "VERIFIED_SUBSTITUTER_ENDPOINT",
  "public_keys": ["VERIFIED_PUBLIC_SIGNING_KEY"]
}
```

Place it under `caches.attic-existing`. Cachix uses `kind: cachix`, `server: null`,
its existing name under `caches.cachix-existing`, and its verified cache URL/keys.
The placeholders above are not valid configuration. Prefer the existing Attic
profile when configured; otherwise select the existing Cachix profile explicitly.
Do not add secrets to this file. Attic/Cachix environment names remain unchanged;
the publisher rejects variable/profile drift. Set `publication_enabled: true`
only through normal review after cache identity verification.

Review closures are untrusted executable content. Digest approval covers the
exact observed selection and bytes, not their correctness. The publisher never
evaluates target Nix while holding secrets. Narrow unsigned import into its fresh
store is necessary because build jobs cannot possess a signing key. No client
configuration or global signature policy is changed. `repo-review fetch` always
checks signatures and appends approved keys; it does not replace default keys.

## Recipe trust

Each `recipes` entry is an exact object containing `repository`, `commit`,
`directory`, `source_input`. A request must match all four fields. The test recipe
is explicitly pinned in policy, using the preserved fixture commit; there is no
built-in bootstrap recipe exemption. Prompt 02 must create a packaging commit and
have its entry reviewed here before dispatch. Source repositories themselves may
be arbitrary public GitHub repositories; their code never becomes controller code.

The effective lock is produced only in the unprivileged stage. Save
`effective.lock`, `effective-plan.json` and the request. For external flakes,
repeat the recorded `--override-input SOURCE_INPUT github:HEAD_REPOSITORY/TESTED_SHA`
when evaluating the original pinned recipe with `--reference-lock-file` and
`--no-update-lock-file`. To remove overrides in a final consumer recipe, commit
both the exact effective lock and the matching source input URL.

## Limits and remaining assurance

Artifact paths reject traversal/symlinks/special files and executable hook
extensions. NAR imports are data operations by pinned Nix, not shell scripts.
Approvals bind the selected root paths as well as all artifact file hashes; an
artifact cannot add arbitrary paths to an already approved digest. This is not
independent reproducible-build attestation. Untrusted evaluation can compromise
its disposable runner and fabricate observations; publication policy must account
for that fact. Do not promote to production substitutes merely because a job is
green. Private cache retrieval requires an independently scoped reader setup;
without it the public-consumer verification fails closed, never compiles locally.
