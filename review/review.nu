use gha.nu *

let inputs = gha review-inputs
let pushToAttic = $inputs.push-to-cache and $env.ATTIC_SERVER != '' and $env.ATTIC_CACHE != ''
let pushToCachix = $inputs.push-to-cache and $env.CACHIX_CACHE != ''
let pr = $env.PR_JSON | from json
let head = $pr.head.sha
let base = $pr.base.sha
let merge = $pr.merge_commit_sha
let jobsArg = if $inputs.builders == "remote" { "-j0" } else { "" }
let system = nix config show system

gha group "install packages" {
  let system = match $system {
    "x86_64-darwin" => "aarch64-darwin"
    _ => $system
  }
  
  [ nixpkgs-review ]
  | if $pushToAttic { $in ++ [ attic-client ] } else { }
  | if $pushToCachix { $in ++ [ cachix ] } else { }
  | each { $".#($in)" }
  | nix profile add --system $system ...$in --builders ''
}

gha group $"run nixpkgs-review ($inputs.extra-args-raw)" {
  cd nixpkgs
  nixpkgs-review -- pr $inputs.pr ...[
    --no-shell
    --no-exit-status
    --no-headers
    --print-result
    --build-args=($"-L ($jobsArg)")
    --pr-json=($env.PR_JSON)
    ...$inputs.extra-args
  ]
}

let reviewDir = $"~/.cache/nixpkgs-review/pr-($inputs.pr)" | path expand
let reportJson = $"($reviewDir)/report.json"

if $pushToAttic or $pushToCachix {
  gha group "push results to cache" {
    let paths = glob $"($reviewDir)/results/*" | path expand
    if ($paths | is-empty) { return }

    let cache = if $pushToAttic {
      attic login default $env.ATTIC_SERVER $env.ATTIC_TOKEN
      try {
        attic cache info $env.ATTIC_CACHE
      } catch {
        gha error "attic returned an error"
        return
      }
      $paths | str join "\n" | attic push --stdin $env.ATTIC_CACHE
      http get -H { Authorization: $"Bearer ($env.ATTIC_TOKEN)" } $"($env.ATTIC_SERVER)_api/v1/cache-config/nixpkgs"
      | select substituter_endpoint public_key is_public
    } else if $pushToCachix {
      with-env { CACHIX_SIGNING_KEY: ($env.CACHIX_SIGNING_KEY | default -e null) } {
        $paths | str join "\n" | cachix push $env.CACHIX_CACHE
        http get -H { Authorization: $"Bearer ($env.ATTIC_TOKEN)" } $"https://app.cachix.org/api/v1/cache/($env.CACHIX_CACHE)"
        | select uri publicSigningKeys isPublic
        | update publicSigningKeys { first }
        | rename substituter_endpoint public_key is_public
      }
    }

    if not $cache.is_public { return }

    $cache | to json | print

    let binaryCaches = [
      "https://cache.nixos.org/"
      $cache.substituter_endpoint
    ]
    let publicKeys = [
      "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
      $cache.public_key
    ]

    [
      $"nix-store -r"
      $"--option binary-caches '($binaryCaches | str join ' ')'"
      $"--option trusted-public-keys '($publicKeys | each { $'(char lf)    ($in)' } | str join)\n  '"
      ...$paths
    ]
    | str join " \\\n  "
    | let fetchCmd
    | print
    $fetchCmd
  }
} | let fetchCmd

open $reportJson
| reject systems
| insert system $system
| insert fetchCmd $fetchCmd
| insert nixConfig { sandbox: (nix config show sandbox) }
| insert head $head
| insert base $base
| insert merge $merge
| update result {
  get -o $system
  | default {}
  | upsert non-existent { default [] }
  | rename -c { non-existent: non_existent }
  | upsert broken { default [] }
  | upsert non_existent { default [] }
  | upsert blacklisted { default [] }
  | upsert failed { default [] }
  | upsert still_failing { default [] }
  | upsert tests { default [] }
  | upsert built { default [] }
  | upsert unsupported { default [] }
}
| let report
| get result
| let result

if $env.IDENTIFY_UNSUPPORTED_PACKAGES == '1' {
  gha group "identify unsupported packages" {
    cd nixpkgs
    if ($result.failed | is-empty) { return $result }

    git fetch origin $merge
    git switch -d $merge

    nix config show --json | save -rf ../nix-config.json
    nix derivation show --recursive -f. ...$result.failed.name | save -rf ../drv-graph.json
    let buildSupport = nix-instantiate ...[
      --eval --strict --json ("../drv-build-support.nix" | path expand)
      --argstr configPath ("../nix-config.json" | path expand)
      --argstr drvGraphPath ("../drv-graph.json" | path expand)
    ] | from json

    $result.failed
    | insert drv { nix eval -f. $"($in.name).drvPath" --raw | path basename }
    | where {|pkg| $buildSupport | get $pkg.drv | not $in.supported }
    | select name aliases
    | let unsupported
    | get name
    | let unsupportedNames

    $result
    | update failed { where name not-in $unsupportedNames }
    | update unsupported { append $unsupported }
  }
} else { $result } | let result
let report = $report | update result $result

if $env.IDENTIFY_STILL_FAILING_PACKAGES == '1' {
  gha group "identify still failing packages" {
    cd nixpkgs
    if ($result.failed | is-empty) { return $result }

    git fetch origin $base
    git switch -d $base

    $result.failed
    | insert path { try { nix eval -f. $"($in.name).outPath" --raw } }
    | where path != null
    | let candidates

    if ($candidates | is-not-empty) {
      try { nix build --keep-going -L ...([$jobsArg] | compact -e) -f. ...$candidates.name }
    }

    $candidates
    | where { nix store verify --no-contents --no-trust $in.path | complete | $in.exit_code != 0 }
    | select name aliases
    | let stillFailing
    | get name
    | let stillFailingNames

    $result
    | update failed { where name not-in $stillFailingNames }
    | update still_failing { append $stillFailing }
  }
} else { $result } | let result
let report = $report | update result $result

gha group "report" {
  $report | save -f report_($system).json
  $report | to json | print
}
