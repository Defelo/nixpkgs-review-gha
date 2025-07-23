# nixpkgs-review-gha

Run [nixpkgs-review](https://github.com/Mic92/nixpkgs-review) in GitHub Actions

## Features
- Build on `x86_64-linux`, `aarch64-linux`, `x86_64-darwin` and `aarch64-darwin`
- No local setup
- Automatically post results on the reviewed pull request
- Optionally start an [Upterm](https://upterm.dev/) session after nixpkgs-review has finished to allow interactive testing/debugging via SSH
- Push new packages to an [Attic](https://github.com/zhaofengli/attic) or [Cachix](https://www.cachix.org/) cache
- Automatically approve or merge pull requests on review success
- Add a "Run nixpkgs-review" shortcut to pull request pages in nixpkgs

## Setup
1. [Fork](https://github.com/Defelo/nixpkgs-review-gha/fork) this repository.
2. In your fork, go to the [Actions](../../actions) tab and enable GitHub Actions workflows.
3. If you don't want to set up [automatic self-updates](#automatic-self-updates-optional), please disable the `self-update` workflow ([Actions / `self-update`](../../actions/workflows/self-update.yml) > `...` button (top right corner) > `Disable workflow`).

### Post Results / Auto Approve/Merge (optional)
If you want nixpkgs-review-gha to automatically post the results on the reviewed pull requests or automatically approve/merge them, you need to generate a [personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens):

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

### Shortcuts on nixpkgs PR pages (optional)
Add [`shortcut.js`](shortcut.js) as a user script in your browser for `https://github.com/` for example using the [User JavaScript and CSS chrome extension](https://chromewebstore.google.com/detail/user-javascript-and-css/nbhcbdghjpllgmfilhnhkllmkecfmpld) or [Violentmonkey](https://violentmonkey.github.io/). Don't forget to update the `repo` constant at the top of the file to point to your fork.

## Usage
1. Open the [review workflow in the "Actions" tab](../../actions/workflows/review.yml)
2. Click on "Run workflow"
3. Enter the number of the pull request in nixpkgs you would like to review and click on "Run workflow"
4. Reload the page if necessary and click on the review run to see the logs
