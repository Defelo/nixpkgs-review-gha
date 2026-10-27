# nixpkgs-review-gha

Run [nixpkgs-review](https://github.com/Mic92/nixpkgs-review) in GitHub Actions

## Features
- Build on `x86_64-linux`, `aarch64-linux`, `x86_64-darwin` and `aarch64-darwin`
- Optionally build on `riscv64-linux` via the [RISE RISC-V Runners](https://riseproject.dev/2026/03/24/announcing-the-rise-risc-v-runners-free-native-risc-v-ci-on-github/) (off by default; requires installing the RISE GitHub App on your fork)
- No local setup
- Automatically post results on the reviewed pull request
- Optionally start an [Upterm](https://upterm.dev/) session after nixpkgs-review has finished to allow interactive testing/debugging via SSH
- Push new packages to an [Attic](https://github.com/zhaofengli/attic) or [Cachix](https://www.cachix.org/) cache
- After a successful review, automatically mark the PR as ready for review, approve it, or merge it (directly or via the [nixpkgs-merge-bot](https://github.com/NixOS/nixpkgs-merge-bot))
- Optionally use [Nix remote builders](https://nix.dev/manual/nix/latest/advanced-topics/distributed-builds) (either in addition to or instead of the local GitHub Actions runner).
- Optionally run the `x86_64-linux` build in your own [GitHub Codespace](https://github.com/features/codespaces) to get past the 6 hour limit of hosted runners (up to 12 hours), automatically when the build is estimated not to fit
- Add a "Run nixpkgs-review" shortcut to pull request pages in nixpkgs

## Setup
1. [Fork](https://github.com/Defelo/nixpkgs-review-gha/fork) this repository.
2. In your fork, go to the [Actions](../../actions) tab and enable GitHub Actions workflows.
3. If you want to set up [automatic self-updates](#automatic-self-updates-optional), please enable the `self-update` workflow ([Actions / `self-update`](../../actions/workflows/self-update.yml) > `...` button (top right corner) > `Enable workflow`).

### Post Results / Auto Approve/Merge (optional)
nixpkgs-review-gha can automatically post the results as a comment on the reviewed pull requests and approve the PR after a successful review out of the box by using an [nrgha-api server](api).
The default instance will use the [@nixpkgs-review-gha](https://github.com/nixpkgs-review-gha) account to create its report comments.
If you want to host and use your own nrgha-api instance, you can [create a new variable](../../settings/variables/actions/new) with the name `API_URL` and set its value to your base URL (without a trailing slash).

If you don't want to use the API server, or if you also want to be able to automatically mark PRs as ready for review or merge them on review success, you need to generate a [personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens):

1. Go to <https://github.com/settings/tokens> and generate a new **classic** token with the `public_repo` scope.
2. In your fork, go to "Settings" > "Secrets and variables" > "Actions" and [add a new repository secret](../../settings/secrets/actions/new) with the name `GH_TOKEN` and set its value to the personal access token you generated before.

> [!WARNING]
> Tokens with the `public_repo` scope have almost full access to *all* public repositories, so they should only be used with great caution.
> Unfortunately it is neither possible to create a classic PAT with a smaller scope nor is it possible to use fine-grained tokens (which aren't *that* fine-grained anyway) here, so it is recommended to not add a `GH_TOKEN` and simply use the nrgha-api server which doesn't require any configuration on your end.

### Automatic Self-Updates (optional)
If you want your fork to update itself on a regular basis, you need to generate a [personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens). Note that this token is different from the one used above!

1. Go to <https://github.com/settings/personal-access-tokens> and generate a new **Fine-grained token** token with access to only your fork ("Repository access" > "Only select repositories") and "Read and write" permissions for both "Contents" and "Workflows".
2. In your fork, go to "Settings" > "Secrets and variables" > "Actions" and [add a new repository secret](../../settings/secrets/actions/new) with the name `GH_SELF_UPDATE_TOKEN` and set its value to the personal access token you generated before.

### Push to Attic Cache (optional)
Follow these steps if you want nixpkgs-review-gha to push new packages to an [Attic](https://github.com/zhaofengli/attic) cache. Replace `$CACHE` with the name of your cache (e.g. `nixpkgs`) and `$SERVER` with the url of your Attic server (e.g. `https://attic.example.com/`):

1. Generate a token with `push` and `pull` permissions: `atticadm make-token --sub nixpkgs-review-gha --validity 1y --pull $CACHE --push $CACHE`
2. [Create a new variable](../../settings/variables/actions/new) with the name `ATTIC_SERVER` and set it to the value of `$SERVER`
3. [Create a new variable](../../settings/variables/actions/new) with the name `ATTIC_CACHE` and set it to the value of `$CACHE`
4. [Create a new secret](../../settings/secrets/actions/new) with the name `ATTIC_TOKEN` and set its value to the token you generated before.

### Push to Cachix (optional)
Follow these steps if you want nixpkgs-review-gha to push new packages to a [Cachix](https://www.cachix.org/) cache. Note: If both an Attic cache and a Cachix cache is configured, the Attic cache is preferred and the Cachix configuration is ignored.

1. Go to https://app.cachix.org/ and set up your binary cache.
2. [Create a new variable](../../settings/variables/actions/new) with the name `CACHIX_CACHE` and set it to the name of your Cachix cache.
3. [Create a new secret](../../settings/secrets/actions/new) with the name `CACHIX_AUTH_TOKEN` and set its value to your auth token. If you are using a self-signed cache, you also need to create a `CACHIX_SIGNING_KEY` secret and set its value to your private signing key.

### Extra Nix Config (optional)
If you have additional configuration you want to append to `/etc/nix/nix.conf`, you can [create a new variable](../../settings/variables/actions/new) with the name `EXTRA_NIX_CONFIG`.
For example, if you want to configure nix to use additional substituters, set its value to the following:

```
extra-substituters = https://nix-community.cachix.org
extra-trusted-public-keys = nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs=
```

### Remote Builders (optional)
It is possible to configure nixpkgs-review-gha to use [remote builders](https://nix.dev/manual/nix/latest/advanced-topics/distributed-builds) either instead of or in addition to the local GitHub Actions runner.
For this to work, the GitHub Actions runner needs to be able to connect to your remote builders via SSH, and you need to configure an SSH keypair for authentication.

Set the following [secrets](../../settings/secrets/actions):

- `SSH_KEY`: A private ssh key which is authorized to access your remote builders. You can generate one using `ssh-keygen -t ed25519 -f ssh_key -N '' -C ''`.
- `SSH_CERT`: If you have configured an [SSH certificate authority](https://manpages.debian.org/unstable/openssh-client/ssh-keygen.1.en.html#CERTIFICATES), the certificate which authorizes your `SSH_KEY` to access the remote builders. You don't need to set this variable if you have authorized your `SSH_KEY` directly (i.e. added your public key to `authorized_keys` on the remote builder).
  <details>
  <summary>Example command to generate a shortlived certificate:</summary>

  ```shell
  ssh-keygen -Us $CA_PUBKEY_PATH \
    -I nixpkgs-review-gha \
    -n $REMOTE_USERNAME \
    -O clear \
    -O force-command="nix-daemon --stdio" \
    -V +1h \
    $PUBKEY_PATH
  ```

  </details>

Set the following [variables](../../settings/variables/actions):

- `BUILDERS`: A newline separated list of build machines in the same format as the [`builders` option in `nix.conf`](https://nix.dev/manual/nix/latest/command-ref/conf-file#conf-builders). You will need to set the value of the third field (ssh identity) to `/etc/nix/ssh_id` which is where your `SSH_KEY` is placed. Your `SSH_CERT` should be picked up automatically, if you have configured one.

For example, you can set `BUILDERS` to the following if you want to build on the [nix-community builders](https://nix-community.org/community-builders/). Keep in mind that these builders should generally [not be trusted](https://nix-community.org/community-builders/#notes-on-security-and-safety), so be careful with what you might push into the [binary caches](#push-to-attic-cache-optional) you configured above.

```
ssh-ng://YOUR_USERNAME@build-box.nix-community.org x86_64-linux /etc/nix/ssh_id 6 - benchmark,big-parallel,kvm,nixos-test,uid-range - c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSUVsSVE1NHFBeTdEaDYzckJ1ZFlLZGJ6SkhycmJyck1YTFlsN1BrbWs4OEg=
ssh-ng://YOUR_USERNAME@aarch64-build-box.nix-community.org aarch64-linux /etc/nix/ssh_id 20 - benchmark,big-parallel,gccarch-armv7-a,gccarch-armv8-a,kvm,nixos-test,uid-range - c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSUc5dXlmaHlsaStCUnRrNjR5K25pcXRiK3NLcXVSR0daODdmNFlSYzhFRTE=
ssh-ng://YOUR_USERNAME@darwin-build-box.nix-community.org aarch64-darwin /etc/nix/ssh_id 2 - big-parallel - c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSUtNSGhsY243ZlVwVXVpT0ZlSWhEcUJ6Qk5Gc2JOcXErTnB6dUdYM2U2enY=
```

### Long-Running Builds in a Codespace (optional)
Jobs on GitHub-hosted runners are killed after 6 hours, which is not enough for some packages (for example Firefox).
For these cases the `x86_64-linux` job of the [review](../../actions/workflows/review.yml) and [build](../../actions/workflows/build.yml) workflows can run in a [GitHub Codespace](https://docs.github.com/en/codespaces) of your own instead.
GitHub only stops a codespace after 12 hours, so the limit is raised to just under 12 hours, and depending on your plan a codespace can offer more cores than a hosted runner.

The `codespace` input of both workflows has three options:
- `auto` (the default) estimates whether the `x86_64-linux` build fits into the 6 hours of a hosted runner and only uses a codespace if it does not, see [below](#how-auto-decides).
  It does nothing as long as the `CODESPACE_TOKEN` secret is not set up, so the workflows behave as before.
- `force` always uses a codespace for `x86_64-linux`, and fails if it cannot be started.
- `disabled` always uses a hosted runner.

The `codespace-size` input selects the machine size of the codespace.
When a codespace is used, the workflow creates it for your fork, starts an ephemeral self-hosted runner in it, runs the `x86_64-linux` job there and deletes the codespace again afterwards.
All other systems keep running on their usual runners.
Because GitHub stops idle codespaces and only connected clients count as activity, the `codespace-guard` jobs keep an SSH session to the codespace open while the build is running.

To set this up, [create a **classic** personal access token](https://github.com/settings/tokens/new) with the `repo` and `codespace` scopes, [create a new secret](../../settings/secrets/actions/new) with the name `CODESPACE_TOKEN` and set its value to the token.
The `repo` scope is needed to register the runner in your fork and the `codespace` scope to create, reach and delete the codespace.
Fine-grained tokens have not been tested.

Codespaces are billed to your account, see [the GitHub documentation](https://docs.github.com/en/billing/concepts/product-billing/github-codespaces) for the included quota and prices.
Which machine sizes are available depends on your plan.
If the one you selected in `codespace-size` is not, the workflow fails early and lists the ones you can use.

#### How `auto` decides
The `estimate` job asks Nix which derivations of the build are not available from a binary cache (`nix build --dry-run`), because only those have to be compiled.
Those are often not the packages you are reviewing but dependencies that no binary cache has yet, for example Chromium for a pull request that touches a single package that uses it.
For reviews, the packages to build are taken from the evaluation of the pull request in the nixpkgs CI, which is the same data that nixpkgs-review uses.

The estimated time on a hosted runner (4 cores) is the sum of the following:
- 5 hours for every derivation that nixpkgs flags as `big-parallel`, which covers the kernel, Chromium, Electron, LibreOffice, Firefox, LLVM, PyTorch and similar
- 3 minutes for every other derivation, or half a minute for downloads and helper derivations that are only assembled locally
- 1.5 times the duration of the newest real build on [Hydra](https://hydra.nixos.org) instead, if there is one for a package with the same name and that is more (a stretch of time that Hydra reports for several packages is only counted once)
- 20 minutes for setting up the job

If the sum exceeds 5 hours, a codespace is used.
To change that limit, [create a variable](../../settings/variables/actions/new) with the name `CODESPACE_ESTIMATE_LIMIT` and the number of minutes as its value.
The numbers behind the decision are shown in the summary of the `estimate` job.
A pull request that rebuilds more than 1500 packages or needs more than 400 derivations to be built is not looked at any further and uses a codespace.

This is only a guess, so keep the following in mind:
- Hydra only reports how long the job of a package itself took.
  It does not know about the dependencies of wrappers like Chromium or LibreOffice and reuses earlier builds instead of building them again, so it can only price part of the packages.
  This is why the `big-parallel` flag of nixpkgs does most of the work.
- A build that hangs cannot be predicted.
- If you select remote builders only (`builders`), the hosted runner does not do the building, so `auto` does not estimate and uses the hosted runner.
- If the nixpkgs CI has not evaluated the pull request yet or the estimate fails, the build uses a hosted runner.
- As soon as the `CODESPACE_TOKEN` secret exists, `auto` adds the time for the estimate (usually a few minutes) before the jobs start.
  Choose `force` or `disabled` to skip it.

Some things to keep in mind:
- Only `x86_64-linux` is supported, because codespaces run on x86_64 machines.
- The `builders` and `upterm` inputs have no effect on the codespace.
  To debug a running build, connect to the codespace with `gh codespace ssh -c <name>` instead (the name is printed in the log of the `codespace` job).
- The Nix store and the build directories live on the large temporary disk of the codespace, but a build that fills it up will still fail.
- The value of the token is only passed to the jobs that manage the codespace, never to the jobs that evaluate or build the pull request.
  The build runs as an unprivileged user without root access and cannot read the secrets of your codespace.
- The workflow deletes the codespace when the job is done, even if it failed or was cancelled.
  As a safety net the codespace is stopped after 30 minutes without a guard connection and deleted 60 minutes after that.
  Codespaces created by the workflow are named `nrgha-<run id>-<attempt>`, so you can find and delete leftovers on the [codespaces page](https://github.com/codespaces).

### Shortcuts on nixpkgs PR pages (optional)
Add [`shortcut.user.js`](shortcut.user.js) as a userscript in your browser for `https://github.com/` for example using the [User JavaScript and CSS chrome extension](https://chromewebstore.google.com/detail/user-javascript-and-css/nbhcbdghjpllgmfilhnhkllmkecfmpld) or [Violentmonkey](https://violentmonkey.github.io/).

The userscript assumes that you forked this repository to your personal account without changing its name. However, if you forked the repo to an organization instead or used a custom repo name or if you would like to use a different repo you have access to, you need to explicitly configure the userscript by updating the `repo` constant at the top of the file to point to the repository you would like to use.

> [!TIP]
> Opening the [raw file](shortcut.user.js?raw=true) with Violentmonkey installed will prompt for installation.

## Usage
1. Open the [review workflow in the "Actions" tab](../../actions/workflows/review.yml)
2. Click on "Run workflow"
3. Enter the number of the pull request in nixpkgs you would like to review and click on "Run workflow"
4. Reload the page if necessary and click on the review run to see the logs

## Experimental Features

### Identify still failing packages
If a package fails to build, try to build the same package again, but on the base branch (e.g. `master` or `release-XX.YY`).
If the package also fails to build on the base branch, it will be marked as "still failing" in the report to make it easier to see which build failures are unlikely to be caused by the PR being reviewed.
Note that these build failures still count towards whether the review is considered successful or not, so if you told nixpkgs-review-gha to automatically approve or merge the PR after a successful review and a package is "still failing" to build (even if all other packages have been built successfully), the PR is *not* approved/merged and you should inspect the build failure manually.

To enable this feature [create a new variable](../../settings/variables/actions/new) with the name `IDENTIFY_STILL_FAILING_PACKAGES` and set its value to `1`.

### Identify unsupported packages
If a package fails to build, try to detect whether the build failure was caused by a missing system feature.
For example, most hosted GitHub Actions runners cannot build QEMU/KVM based NixOS tests because they lack the `kvm` feature.
Trying to build a NixOS test on such a runner will always fail, so with this experimental feature enabled, packages that fail to build due to a missing system feature will be marked as "unsupported" and will NOT count towards review success/failure, i.e. if some packages are unsupported and all other packages have been built successfully, the PR *is* approved/merged automatically if you told nixpkgs-review-gha to do that.

This feature is enabled by default. To disable it [create a new variable](../../settings/variables/actions/new) with the name `IDENTIFY_UNSUPPORTED_PACKAGES` and set its value to `0`.
