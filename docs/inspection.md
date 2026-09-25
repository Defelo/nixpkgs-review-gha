# Starting state and sources

Inspected 2026-09-25/26 before editing. Isolated branch
`feat/repository-review-service`; no new GitHub fork was created.

- Existing fork: `caniko/nixpkgs-review-gha`, default branch `main`, starting
  `8bcdcade8a688762891c11b6b4009538277b31cb`.
- Actual Defelo upstream fetched from GitHub:
  `05be2410ba6775b612fe733127703ec71b3b3425`.
- Common fork baseline includes `9d840c2` and pinned nixpkgs
  `13043924aaa7375ce482ebe2494338e058282925`.
- The only fork-only commit relative to current upstream was `8bcdcad`, an empty
  “Initialize Actions” commit. History and author metadata are retained.
- No repository-specific AGENTS.md existed. The parent canix worktree contained
  unrelated work, which was left alone; this infrastructure has its own worktree.

Read README, flake/lock, every workflow, setup-nix, report/publication code and
userscript. Confirmed hardcoded NixOS/nixpkgs API/checkouts/links/posts, unbounded
mergeability polling, cache secrets in the build job, missing Markdown → “No
rebuilds,” macos-latest for both architectures, and unattended rebase followed by
`git push --force-with-lease`.

Current upstream was compared, not silently merged. It adds a Nushell workflow,
an API/OIDC service, unsupported/still-failing package classification, riscv64
runner support, and removes x86_64-darwin from current flake checks. This task
retains the fork's four-system contract with explicit labels/architecture checks
and uses a compact local CLI instead of introducing that upstream server.

Only configuration **names** and non-secret public variables were inspected:

- `ATTIC_SERVER`: `https://attic.candee.baby/`
- `ATTIC_CACHE`: `nixpkgs-review-gha`
- Secret names present: `ATTIC_TOKEN`, `GH_SELF_UPDATE_TOKEN`, `GH_TOKEN`.
- No Cachix variables were configured at inspection time.

No secret value was read, printed or committed. No signing key was inferred from
the endpoint. The empty cache policy intentionally blocks publication until an
operator verifies and reviews the actual existing URL/public key profile.

## Verified references

- <https://github.com/caniko/nixpkgs-review-gha>
- <https://github.com/Defelo/nixpkgs-review-gha> at the commit above
- <https://github.com/Mic92/nixpkgs-review/tree/3.7.0>: read `cli/rev.py`,
  `review.py`, `report.py`, `builddir.py`, and `nix.py`. The PR CLI's merge fetch
  even for head checkout motivated the pinned selection-only API adapter.
- <https://raw.githubusercontent.com/NixOS/nixpkgs/13043924aaa7375ce482ebe2494338e058282925/pkgs/by-name/ni/nixpkgs-review/package.nix>
- <https://docs.github.com/en/actions/reference/security/secure-use>
- <https://docs.github.com/en/actions/concepts/security/github_token>
- <https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows>
- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-copy>
- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-build>

External action refs were resolved via GitHub's Git refs API to full release
commit SHAs. Checkout disables persistent credentials; artifact actions transfer
data only; the installer uses an explicit Nix release URL. Future updates must
review upstream changes and maintain full-SHA pins.
