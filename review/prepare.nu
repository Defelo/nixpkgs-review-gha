use gha.nu *

let inputs = gha review-inputs

if $env.HAS_GH_TOKEN == '1' {
  gha group "warn about deprecated GH_TOKEN secret" {
    gha warning --title "Deprecated GH_TOKEN secret found!" "nixpkgs-review-gha can now post the review results out of the box, so this secret is no longer needed for that. It is recommended to revoke your token and remove the GH_TOKEN secret. See https://github.com/Defelo/nixpkgs-review-gha#post-results--auto-approvemerge-optional for more information."
  }
}

gha group "display inputs" {
  $inputs | to json | print
}

gha group "get pr" {
  mut pr = {}
  loop {
    $pr = ^gh api $"/repos/NixOS/nixpkgs/pulls/($inputs.pr)" | from json
    if $pr.mergeable_state != "unknown" or $pr.merged { break }
    print "mergeable state not known yet, retrying..."
    sleep 2sec
  }

  $pr | to json | print

  if not ($pr.merged or $pr.mergeable) {
    gha error "PR is not mergeable"
  }

  $pr | to json -r | gha output pr
}
