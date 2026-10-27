use ../review/gha.nu *
use estimate-lib.nu *

# Decide whether the x86_64-linux build is likely to run into the 6 hour limit of a hosted runner.
# The environment provides:
#   WORKFLOW        review or build
#   PR_JSON         pull request of the review workflow
#   PACKAGES        installables of the build workflow
#   ALLOW_UNFREE    true or false
#   ALLOW_INSECURE  true or false
#   LIMIT_MINUTES   estimated minutes above which a codespace is used (optional)
#   GH_TOKEN        token to read the results of the nixpkgs CI (review only)

# More packages or derivations than this are assumed to be too much for a hosted runner without looking closer
const max_attrs = 1500
const max_drvs = 400
const max_hydra_lookups = 40

# Nix expression that collects the packages of a pull request into one derivation, so that a single dry-run covers all
const review_expression = r#'
let
  pkgs = import (builtins.fetchTarball "https://github.com/NixOS/nixpkgs/archive/@REV@.tar.gz") {
    system = "x86_64-linux";
    config = {
      allowUnfree = @UNFREE@;
      allowInsecurePredicate = _: @INSECURE@;
    };
  };
  get =
    name:
    let
      result = builtins.tryEval (
        let
          p = pkgs.lib.attrByPath (pkgs.lib.splitString "." name) null pkgs;
        in
        if p != null && pkgs.lib.isDerivation p && (p.meta.available or true) && !(p.meta.broken or false) then builtins.seq p.drvPath p else null
      );
    in
    if result.success then result.value else null;
  drvs = builtins.filter (p: p != null) (map get (builtins.fromJSON (builtins.readFile @ATTRS@)));
in
pkgs.runCommand "review-estimate" { buildInputs = drvs; } "touch $out"
'#

def conclude [codespace: bool, reason: string] {
  gha notice --title (if $codespace { "Codespace" } else { "Hosted runner" }) $reason
  $codespace | into string | gha output codespace
  exit 0
}

def format-minutes [minutes: number] {
  $"(($minutes / 60) | math floor)h (($minutes mod 60) | math round)min"
}

# Names of the packages that the pull request rebuilds on x86_64-linux according to the nixpkgs CI, or null
def rebuilt-attrs [pr: record] {
  let runs = ^gh api $"/repos/NixOS/nixpkgs/actions/runs?head_sha=($pr.head.sha)&per_page=50" | from json | get workflow_runs | where name in [PR Eval]
  for run in $runs {
    let artifacts = ^gh api $run.artifacts_url | from json | get artifacts | where name == comparison
    if ($artifacts | is-empty) { continue }
    let dir = ^mktemp --directory | str trim
    ^gh api $"/repos/NixOS/nixpkgs/actions/artifacts/($artifacts | first | get id)/zip" | save --raw $"($dir)/comparison.zip"
    if (which unzip | is-not-empty) {
      ^unzip -q -o $"($dir)/comparison.zip" -d $dir
    } else {
      ^python3 -m zipfile -e $"($dir)/comparison.zip" $dir
    }
    let changed = open $"($dir)/changed-paths.json"
    let rebuilds = $changed.rebuildsByPlatform?."x86_64-linux"? | default []
    let added = $changed.attrdiffByPlatform?."x86_64-linux"?.added? | default []
    return ($rebuilds ++ $added | uniq)
  }
  null
}

# Run nix build --dry-run for the packages of a pull request
def dry-run-review [pr: record, attrs: list<string>] {
  let dir = ^mktemp --directory | str trim
  $attrs | to json | save --raw $"($dir)/attrs.json"
  $review_expression
  | str replace '@REV@' ($pr.merge_commit_sha? | default $pr.head.sha)
  | str replace '@UNFREE@' $env.ALLOW_UNFREE
  | str replace '@INSECURE@' $env.ALLOW_INSECURE
  | str replace '@ATTRS@' $"($dir)/attrs.json"
  | save --raw $"($dir)/estimate.nix"
  ^nix build --dry-run --no-link --keep-going -f $"($dir)/estimate.nix" | complete
}

# Run nix build --dry-run for the installables of the build workflow
def dry-run-build [packages: list<string>] {
  let impure = if ($env.ALLOW_UNFREE == "true") or ($env.ALLOW_INSECURE == "true") { ["--impure"] } else { [] }
  let config = {
    NIXPKGS_ALLOW_UNFREE: (if $env.ALLOW_UNFREE == "true" { "1" } else { "0" })
    NIXPKGS_ALLOW_INSECURE: (if $env.ALLOW_INSECURE == "true" { "1" } else { "0" })
  }
  with-env $config { ^nix build --dry-run --no-link --keep-going ...$impure ...$packages | complete }
}

def main [] {
  let limit = $env.LIMIT_MINUTES? | default "" | if ($in | is-empty) { 300 } else { try { $in | into int } catch { 300 } }

  let dry = if $env.WORKFLOW == "review" {
    let pr = $env.PR_JSON | from json
    let attrs = gha group "get the rebuilt packages from the nixpkgs CI" { rebuilt-attrs $pr }
    if $attrs == null {
      conclude false "The nixpkgs CI has no evaluation for this pull request yet, so the build time cannot be estimated and a hosted runner is used."
    }
    print $"($attrs | length) packages are rebuilt on x86_64-linux"
    if ($attrs | is-empty) { conclude false "Nothing is rebuilt on x86_64-linux." }
    if ($attrs | length) > $max_attrs {
      conclude true $"($attrs | length) packages are rebuilt, which is a mass rebuild that will not fit into a hosted runner."
    }
    gha group "check what has to be built" { dry-run-review $pr $attrs }
  } else {
    let packages = $env.PACKAGES | split row -r '\s+' | where {|p| $p | is-not-empty }
    if ($packages | is-empty) { conclude false "No packages to build." }
    gha group "check what has to be built" { dry-run-build $packages }
  }

  if $dry.exit_code != 0 {
    print $dry.stderr
    conclude false "Nix could not work out what has to be built, so the build time cannot be estimated and a hosted runner is used."
  }

  let drvs = $dry.stderr | dry-run drvs | where {|d| $d !~ '-review-estimate\.drv$' }
  print $"($drvs | length) derivations have to be built"
  if ($drvs | is-empty) { conclude false "Everything is available from a binary cache." }
  if ($drvs | length) > $max_drvs {
    conclude true $"($drvs | length) derivations have to be built, which will not fit into a hosted runner."
  }

  let infos = gha group "inspect the derivations" { drv info $drvs }
  let prelim = estimate $infos {} $default_weights

  # Hydra is only asked when the estimate does not already exceed the limit
  let hydra = if $prelim.minutes > $limit {
    {}
  } else {
    gha group "look up build times on Hydra" {
      $infos
      | where {|i| not $i.fixed_output and not $i.big_parallel }
      | get hname
      | uniq
      | first $max_hydra_lookups
      | hydra lookup $in
    }
  }
  let final = estimate $infos $hydra $default_weights

  [
    "## Build time estimate for a hosted runner"
    ""
    $"($final.derivations) derivations have to be built: ($final.big_parallel) big-parallel, ($final.fixed_output) fixed-output and ($final.local_only) local-only."
    $"Estimated time: **(format-minutes $final.minutes)**, a codespace is used above (format-minutes $limit)."
    ""
    "The most expensive derivations, in minutes:"
    ""
    ($final.top | select name minutes | update minutes {|r| $r.minutes | math round } | to md --pretty)
  ] | str join "\n" | gha step-summary

  if $final.minutes > $limit {
    let note = if $final.minutes > 720 { " That is more than the 12 hours a codespace can run, so it will probably not finish there either." } else { "" }
    conclude true $"The build is estimated to take (format-minutes $final.minutes) on a hosted runner, so a codespace is used.($note)"
  } else {
    conclude false $"The build is estimated to take (format-minutes $final.minutes) on a hosted runner, which fits into its limit."
  }
}

try { main } catch {|err|
  print $err.rendered
  conclude false $"The estimate failed, so a hosted runner is used: ($err.msg)"
}
