use gha.nu *

let inputs = gha review-inputs
let pr = $env.PR_JSON | from json
let head = $pr.head.sha
let base = $pr.base.sha
let base_ref = $pr.base.ref
let merge = $pr.merge_commit_sha

let systems = [
  x86_64-linux
  aarch64-linux
  x86_64-darwin
  aarch64-darwin
  riscv64-linux
]

gha group "generate report" {
  let reports = $systems | each { try { open $"report_($in).json" } }

  $reports | to json | print
  $reports | to json | save -f reports.json

  $reports.result
  | all { select failed still_failing | values | compact | flatten | is-empty }
  | let success
  | if $in { ":white_check_mark:" } else { ":x" }
  | let icon

  mut nixpkgsReviewCmd = $"nixpkgs-review pr ($inputs.pr)"
  if ($inputs.extra-args-raw | is-not-empty) {
    $nixpkgsReviewCmd += $" ($inputs.extra-args-raw)"
  }

  mut report = ""
  $report += $"## ($icon) `nixpkgs-review` result\n\n"
  $report += $"Generated using [`nixpkgs-review-gha`]\(https://github.com/Defelo/nixpkgs-review-gha) \([`($env.SHA | str substring ..<7)`]\(https://github.com/Defelo/nixpkgs-review-gha/commit/($env.SHA)))\n\n"
  $report += $"Command: `($nixpkgsReviewCmd)`\n"
  $report += $"Commit: [`($head)`]\(https://github.com/NixOS/nixpkgs/commit/($head)) \([subsequent changes]\(https://github.com/NixOS/nixpkgs/compare/($head)..pull/($inputs.pr)/head))\n"
  $report += $"Merge: [`($merge)`]\(https://github.com/NixOS/nixpkgs/commit/($merge))\n\n"
  $report += $"Logs: https://github.com/($env.REPO)/actions/runs/($env.RUN_ID)/attempts/($env.RUN_ATTEMPT)\n\n"

  $reports
  | where ($it.fetchCmd | is-not-empty)
  | each { $"<li><details><summary><code>($in.system)</code></summary>\n\n```shell\n($in.fetchCmd)\n```\n</details></li>\n" }
  | str join
  | if ($in | is-not-empty) {
    $report += $"<details><summary>Download packages from cache:</summary><ul>\n($in)</ul></details>\n\n"
  }

  let htmlPkgsSection = {|emoji, packages, msg, what = "package"|
    if ($packages | is-empty) { return "" }
    let plural = if ($packages | length) > 1 { "s" } else { "" }
    $packages
    | each {|pkg|
      $pkg.name
      | if ($pkg.aliases | is-not-empty) { $"($in) \(($pkg.aliases | str join ', '))" } else { }
      | $"    <li>($in)</li>\n"
    }
    | str join
    | $"<details>\n  <summary>($emoji) ($packages | length) ($what)($plural) ($msg):</summary>\n  <ul>\n($in)  </ul>\n</details>\n"
  }

  for it in $reports {
    let hasRebuilds = $it.result | values | flatten | is-not-empty
    let systemSuffix = if $it.system !~ '-linux$' and $hasRebuilds { $" \(sandbox = ($it.nixConfig.sandbox))" }

    $report += "\n---\n"
    $report += $"### `($it.system)`($systemSuffix)\n"
    $report += do $htmlPkgsSection ":fast_forward:" $it.result.broken "marked as broken and skipped"
    $report += do $htmlPkgsSection ":fast_forward:" $it.result.non_existent "present in ofBorgs evaluation, but not found in the checkout"
    $report += do $htmlPkgsSection ":fast_forward:" $it.result.blacklisted "blacklisted"
    $report += do $htmlPkgsSection ":x:" $it.result.failed "failed to build"
    $report += do $htmlPkgsSection ":x:" $it.result.still_failing $"still failing to build \(also failed on ($base_ref))"
    $report += do $htmlPkgsSection ":white_check_mark:" $it.result.tests "built" "test"
    $report += do $htmlPkgsSection ":white_check_mark:" $it.result.built "built"
    $report += do $htmlPkgsSection ":grey_question:" $it.result.unsupported "not supported on this runner"
    if not $hasRebuilds { $report += ":white_check_mark: *No rebuilds*\n" }
  }

  print $report
  $report | save -f report.md

  if $success { print "SUCCESS" } else { print "FAILURE" }

  $report
  | str replace -r '^.*' $"$0 for [#($inputs.pr)]\(https://github.com/NixOS/nixpkgs/pull/($inputs.pr))"
  | gha step-summary

  {
    reports: $reports
    report: $report
    success: $success
  }
} | let review

if ($env.GH_TOKEN | is-empty) {
  if not $inputs.post-result and $inputs.on-success == 'nothing' { exit; }

  gha group "submit reports to api" {
    let api_url = $env.API_URL | default -e "https://nrgha-api.defelo.de"
    let audience = http get $"($api_url)/oidc_client_id"
    let oidc_token = gha get-oidc-token $audience

    $review.reports
    | select system nixConfig fetchCmd result
    | update nixConfig { select sandbox }
    | flatten nixConfig result
    | rename -c { fetchCmd: fetch_cmd }
    | {
      pr: $inputs.pr,
      extra_args: $inputs.extra-args-raw,
      head: $head,
      merge: $merge,
      base_ref: $base_ref,
      systems: $in,
      post_result: $inputs.post-result,
      on_success: $inputs.on-success,
    }
    | http post $"($api_url)/submit_report" -H {Authorization: $"Bearer ($oidc_token)"} -t application/json
    | let response

    $response | to json | print
    
    if ($response.errors | is-not-empty) {
      $response.errors | each { gha error $"API error: ($in)" }
      exit 1
    }
  }

  exit
}

if $inputs.post-result {
  gha group "post comment" {
    gh pr -R NixOS/nixpkgs comment $inputs.pr -b $review.report
  }
}

if not $review.success { exit }

if $inputs.on-success == 'mark_as_ready' {
  gha group "mark pull request as ready for review" {
    gh pr -R NixOS/nixpkgs ready $inputs.pr
  }
}

if $inputs.on-success in [approve, merge] {
  gha group "approve pull request" {
    let user_id = gh api /user | from json | get id
    if $user_id != $pr.user.id {
      gh pr -R NixOS/nixpkgs review $inputs.pr --approve -b "Approved automatically following the successful run of `nixpkgs-review`."
    } else if $inputs.on-success == 'approve' {
      gha error "You cannot approve your own pull request."
      exit 1
    }
  }
}

if $inputs.on-success == 'merge' {
  gha group "merge pull request" {
    let is_committer = gh api /repos/NixOS/nixpkgs | from json | get permissions.push
    if $is_committer {
      gh pr -R NixOS/nixpkgs merge $inputs.pr --merge --match-head-commit $head
    } else {
      let current_head = gh api /repos/NixOS/nixpkgs/pulls/($inputs.pr) | from json | get head.sha
      if $current_head != $head {
        gha error $"Refusing to merge because the head branch was modified \(expected ($head), got ($current_head) instead)"
        exit 1
      }
      gh pr -R NixOS/nixpkgs comment $inputs.pr -b "@NixOS/nixpkgs-merge-bot merge"
    }
  }
}
