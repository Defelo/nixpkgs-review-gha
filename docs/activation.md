# Activation and Prompt 02 handoff

The implementation PR must be reviewed and merged manually. This task is not
authorized to merge, alter branch protections, activate hosts, change billing,
or provision paid runners. New `workflow_dispatch` files must exist on the fork's
default branch before normal use. Local Nix evaluation/builds were explicitly
declined by the operator; Nix gates run in `check.yml` only.

After the PR is merged, run these Bash commands from the controller checkout:

```bash
CONTROLLER=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
REVISION=$(gh api "repos/$CONTROLLER/commits/main" --jq .sha)
nix build "github:$CONTROLLER/$REVISION#repo-review" --out-link tools
tools/bin/repo-review example | jq \
  --arg repo "$CONTROLLER" --arg rev "$REVISION" \
  '.repository=$repo | .pr=null | .revision=$rev | .directory="fixtures/flake" | .test_profile="checks-rebuild-v1"' > acceptance.json
tools/bin/repo-review dispatch acceptance.json --controller "$CONTROLLER" --revision "$REVISION"
gh run list --repo "$CONTROLLER" --workflow review-repository.yml --commit "$REVISION" \
  --json databaseId,headSha,status,conclusion
# Set RUN_ID from the matching dispatch; do not guess or silently choose a concurrent run.
tools/bin/repo-review status --controller "$CONTROLLER" --run "$RUN_ID" --wait-seconds 300
tools/bin/repo-review report --controller "$CONTROLLER" --run "$RUN_ID" --attempt 1 --output acceptance-report
jq '{build,tests,closure_export,publication,retrieval,missing_platforms}' acceptance-report/review-result.json
```

Acceptance requires `build: passed`, `tests: passed`, `closure_export: passed`, no missing platforms,
exact identities, and `explicit-check-rebuild` evidence. Publication and retrieval
remain `not_run` for this request. Inspect artifacts and logs; don't infer service
readiness from the PR/check YAML alone.

For external acceptance, use the same exact controller commit as target and the
exact fixture recipe entry reviewed in `policy.json`, backend `external-flake`,
directory `.`, and the recipe's recorded directory and source input. There is no
implicit fixture-policy exemption. CI covers conventional, external and deliberately failing
fixtures and secretless isolated closure transfer.

After trusted cache profile activation, submit a request with
`publication: request-approval` and the named existing profile. Inspect its report,
effective plan and artifacts. Explicit promotion (one platform at a time):

```bash
gh workflow run publish-review.yml --repo "$CONTROLLER" --ref main \
  -f run_id="$RUN_ID" -f attempt="$ATTEMPT" -f revision="$REVISION" \
  -f system=x86_64-linux -f plan_digest="$EFFECTIVE_PLAN_DIGEST" \
  -f bundle_digest="$BUNDLE_DIGEST"
```

Promotion requires a successful source run and every requested platform's build
job to have succeeded in that exact attempt. A partial platform success, failed
export, skipped/cancelled job, or forged aggregate cannot be promoted.

Verify both `publication-…` and `retrieval-…` receipts and the promotion run's
conclusion. A publication receipt alone does not prove fresh-store availability.

## Prompt 02

Use `examples/openpencil-external.template.json`. Its `REPLACE_*` commits and
recipe coordinates are unresolved placeholders and intentionally fail validation.
Prompt 02 creates those exact commits, supplies package/check output names,
registers the recipe pin through policy review, then uses the generated request
schema and CLI validation. OpenPencil is an ordinary external-flake client. This
task makes no OpenPencil changes or upstream posts.

The infrastructure draft is [PR #1](https://github.com/caniko/nixpkgs-review-gha/pull/1).
The engine migration is [Simit PR #31](https://github.com/caniko/simit/pull/31).
Review both together: this controller pins an exact migration commit through a
direct, non-flake Simit source input. Its content hash is captured by the CI lock
receipt, and its tool package embeds the engine identity checked against plans.
Merge the engine migration manually before the controller, retaining that
reviewed exact pin until a separately reviewed update.
The checked-in discovery descriptor pins a complete implementation commit; its
`ready: false` describes activation status. Its controller pin deliberately
precedes the documentation commit containing the descriptor, avoiding a
self-referential commit hash. Resolve the reviewed default-branch commit after
merge and regenerate the descriptor for the live acceptance run.

`repo-review manifest --controller "$CONTROLLER" --revision "$REVISION"` generates
the actual service discovery document. Re-generate it after activation with real
verification evidence; `ready: false` is deliberate until those gates complete.
