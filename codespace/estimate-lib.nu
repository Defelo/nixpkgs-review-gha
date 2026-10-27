# Estimate how long a build takes on a GitHub-hosted runner, which has 4 cores and is stopped after 6 hours.
#
# The estimate is based on the derivations that `nix build --dry-run` reports as to be built, which are exactly the
# ones that are missing from the binary caches.  Derivations that nixpkgs flags as `big-parallel` are assumed to be
# huge (the kernel, Chromium, Electron, LibreOffice, ...).  All others get a small default, which is raised for
# packages for which Hydra knows how long they took.

# Assumed minutes on a hosted runner and the factor to apply to the duration reported by Hydra
export const default_weights = {
  big_parallel: 300
  drv: 3
  cheap: 0.5
  hydra_factor: 1.5
  overhead: 20
}

# Parse the derivation paths out of the output of `nix build --dry-run`
export def "dry-run drvs" []: string -> list<string> {
  let lines = $in | lines
  let header = $lines | enumerate | where {|l| $l.item =~ '^(this derivation|these [0-9]+ derivations) will be built:' }
  if ($header | is-empty) { return [] }
  $lines | skip (($header | first | get index) + 1) | take while {|l| $l =~ '^\s+/nix/store/\S+\.drv\s*$' } | each { str trim }
}

# Name of a derivation without its store path prefix and version suffix
def pname-of [name: string] {
  $name | str replace -r '-[0-9].*$' ''
}

# Inspect derivations and report for each whether it is fixed-output and big-parallel
export def "drv info" [drvs: list<string>] {
  if ($drvs | is-empty) { return [] }
  let shown = ^nix derivation show ...$drvs | from json
  let table = if ("derivations" in ($shown | columns)) { $shown.derivations } else { $shown }
  $table | transpose drv info | each {|row|
    let info = $row.info
    let drv_env = $info.env? | default {}
    # Structured attributes live in a separate field with Nix and in env.__json with Lix
    let attrs = if ($info.structuredAttrs? | is-not-empty) {
      $info.structuredAttrs
    } else if ("__json" in ($drv_env | columns)) {
      $drv_env.__json | from json
    } else {
      $drv_env
    }
    let features = $attrs.requiredSystemFeatures? | default [] | if ($in | describe | str starts-with "list") { $in } else { $in | into string | split row -r '\s+' }
    let name = $info.name? | default ($row.drv | str replace -r '^.*/[a-z0-9]+-' '' | str replace -r '\.drv$' '')
    {
      drv: $row.drv
      name: $name
      pname: ($attrs.pname? | default (pname-of $name))
      hname: (pname-of $name)
      fixed_output: ($info.outputs | values | any {|o| ($o.hash? | is-not-empty) })
      # Derivations of helpers like writeText or buildEnv are only assembled locally and take seconds
      local_only: (($attrs.preferLocalBuild? | default false | into string) in ["1" "true"])
      big_parallel: ("big-parallel" in $features)
    }
  }
}

# Estimated minutes on a hosted runner for one derivation, given the minutes Hydra reported for it (or null)
export def "drv minutes" [info: record, hydra: any, weights: record] {
  if $info.fixed_output or $info.local_only { return $weights.cheap }
  let base = if $info.big_parallel { $weights.big_parallel } else { $weights.drv }
  let known = if $hydra == null { 0 } else { $hydra * $weights.hydra_factor }
  [$base $known] | math max
}

# Names that are unlikely to have a Hydra job of their own, so looking them up would only waste requests
def skip-hydra [pname: string] {
  $pname =~ '^(python|perl|ruby|lua|ocaml|haskell|emacs|vim|rust|go|nodejs)[0-9._]*-' or $pname =~ '(-env|-wrapper|-wrapped|-hook|-vendor|-deps|-modules|-staging|^source)$'
}

# Name of the Hydra job that builds a package for x86_64-linux, or null if there probably is none
def hydra-job [pname: string] {
  if (skip-hydra $pname) { return null }
  let attr = if ($pname =~ '^libretro-') { $pname | str replace 'libretro-' 'libretro.' } else { $pname }
  $"nixpkgs.($attr).x86_64-linux"
}

# The newest build of the package on Hydra that was actually built instead of reused, as its minutes and the time
# window it covers, or null.  Hydra gives builds that were part of one chain of work the same window.
export def "hydra build" [name: string] {
  let job = hydra-job $name
  if $job == null { return null }
  let headers = { Accept: "application/json", User-Agent: "nixpkgs-review-gha" }
  let builds = try {
    http get --max-time 15sec -H $headers $"https://hydra.nixos.org/api/latestbuilds?nr=6&project=nixos&jobset=unstable&job=($job)"
  } catch { [] }
  for build in $builds {
    # A build that Hydra reused has the same start and stop time
    let details = try { http get --max-time 15sec -H $headers $"https://hydra.nixos.org/build/($build.id)" } catch { null }
    if $details == null { continue }
    let seconds = $details.stoptime - $details.starttime
    if $seconds > 60 { return { minutes: ($seconds / 60), window: $"($details.starttime)-($details.stoptime)" } }
  }
  null
}

# Look up Hydra for the given package names, answering a record from name to its build for those that are known
export def "hydra lookup" [names: list<string>] {
  $names | uniq | par-each --threads 8 {|name| { name: $name, build: (hydra build $name) } }
  | where build != null
  | reduce --fold {} {|it, acc| $acc | insert $it.name $it.build }
}

# Total estimate in minutes and the most expensive derivations, given inspected derivations and known Hydra builds.
# A window of Hydra is only counted once for different packages, because they share the work in it.
export def "estimate" [infos: list<record>, hydra: record, weights: record] {
  let priced = $infos | reduce --fold { shared: [], rows: [] } {|info, acc|
    let build = $hydra | get -o $info.hname
    let fresh = $build != null and ($acc.shared | where window == $build.window and hname != $info.hname | is-empty)
    let minutes = drv minutes $info (if $fresh { $build.minutes } else { null }) $weights
    {
      shared: (if $fresh { $acc.shared | append { window: $build.window, hname: $info.hname } } else { $acc.shared })
      rows: ($acc.rows | append ($info | insert minutes $minutes))
    }
  } | get rows
  {
    minutes: ((if ($priced | is-empty) { 0 } else { $priced.minutes | math sum }) + $weights.overhead)
    derivations: ($priced | length)
    big_parallel: ($priced | where big_parallel | length)
    fixed_output: ($priced | where fixed_output | length)
    local_only: ($priced | where local_only | length)
    top: ($priced | sort-by minutes --reverse | first 10)
  }
}
