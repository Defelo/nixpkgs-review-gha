# nixpkgs-review-gha

Run [nixpkgs-review](https://github.com/Mic92/nixpkgs-review) in GitHub Actions

## Features
- Build on `x86_64-linux`, `aarch64-linux`, `x86_64-darwin` and `aarch64-darwin`
- No local setup
- Automatically post results on the reviewed pull request
- Optionally start an [Upterm](https://upterm.dev/) session after nixpkgs-review has finished to allow interactive testing/debugging via SSH
- Push new packages to an [Attic](https://github.com/zhaofengli/attic) or [Cachix](https://www.cachix.org/) cache
- After a successful review, automatically mark the PR as ready for review, approve it, or merge it (directly or via the [nixpkgs-merge-bot](https://github.com/NixOS/nixpkgs-merge-bot))
- Optionally use [Nix remote builders](https://nix.dev/manual/nix/latest/advanced-topics/distributed-builds) (either in addition to or instead of the local GitHub Actions runner).
- Add a "Run nixpkgs-review" shortcut to pull request pages in nixpkgs

## Setup
1. [Fork](https://github.com/Defelo/nixpkgs-review-gha/fork) this repository.
2. In your fork, go to the [Actions](../../actions) tab and enable GitHub Actions workflows.
3. If you want to set up [automatic self-updates](#automatic-self-updates-optional), please enable the `self-update` workflow ([Actions / `self-update`](../../actions/workflows/self-update.yml) > `...` button (top right corner) > `Enable workflow`).

### Post Results / Auto Approve/Merge (optional)
If you want nixpkgs-review-gha to automatically post the results on the reviewed pull requests or automatically mark them as ready for review or approve/merge them, you need to generate a [personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens):

1. Go to <https://github.com/settings/tokens> and generate a new **classic** token with the `public_repo` scope.
2. In your fork, go to "Settings" > "Secrets and variables" > "Actions" and [add a new repository secret](../../settings/secrets/actions/new) with the name `GH_TOKEN` and set its value to the personal access token you generated before.

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
- `USE_BUILDERS`: Either `no`, `yes`, or `always`. If set to `yes`, remote builders are used *in addition to* the GitHub Actions runner. If set to `always`, *only* remote builders are used and no builds happen on the runner. If set to `no`, remote builders are not used at all.

For example, you can set `BUILDERS` to the following if you want to build on the [nix-community builders](https://nix-community.org/community-builders/). Keep in mind that these builders should generally [not be trusted](https://nix-community.org/community-builders/#notes-on-security-and-safety), so be careful with what you might push into the [binary caches](#push-to-attic-cache-optional) you configured above.

```
ssh-ng://YOUR_USERNAME@build-box.nix-community.org x86_64-linux /etc/nix/ssh_id 6 - benchmark,big-parallel,kvm,nixos-test,uid-range - c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSUVsSVE1NHFBeTdEaDYzckJ1ZFlLZGJ6SkhycmJyck1YTFlsN1BrbWs4OEg=
ssh-ng://YOUR_USERNAME@aarch64-build-box.nix-community.org aarch64-linux /etc/nix/ssh_id 20 - benchmark,big-parallel,gccarch-armv7-a,gccarch-armv8-a,kvm,nixos-test,uid-range - c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSUc5dXlmaHlsaStCUnRrNjR5K25pcXRiK3NLcXVSR0daODdmNFlSYzhFRTE=
ssh-ng://YOUR_USERNAME@darwin-build-box.nix-community.org x86_64-darwin,aarch64-darwin /etc/nix/ssh_id 2 - big-parallel - c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSUtNSGhsY243ZlVwVXVpT0ZlSWhEcUJ6Qk5Gc2JOcXErTnB6dUdYM2U2enY=
```

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
