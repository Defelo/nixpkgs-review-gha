use ../review/gha.nu *
use lib.nu *

# Codespaces machine types by the core count offered in the workflow inputs
let machines = {
  "2-core": basicLinux32gb
  "4-core": standardLinux32gb
  "8-core": premiumLinux
  "16-core": largePremiumLinux
  "32-core": xLargePremiumLinux
}

let label = cs label
let machine = $machines | get -o $env.MACHINE

if ($env.GH_TOKEN | is-empty) {
  gha error --title "Missing CODESPACE_TOKEN secret" "Running builds in a codespace requires a personal access token, see the README for how to set up the CODESPACE_TOKEN secret."
  exit 1
}

if $machine == null {
  gha error $"Unknown codespace machine size '($env.MACHINE)'"
  exit 1
}

gha group "check machine availability" {
  let available = try {
    cs api GET $"/repos/($env.REPO)/codespaces/machines?ref=($env.REF)" | get machines.name
  } catch { [] }

  if ($available | is-not-empty) and $machine not-in $available {
    gha error --title "Machine size not available" $"Your account cannot create a ($env.MACHINE) codespace for this repository. Available machine types: ($available | str join ', ')"
    exit 1
  }
}

let release = gha group "find latest actions runner" {
  let release = ^gh api /repos/actions/runner/releases/latest | from json
  let version = $release.tag_name | str replace -r '^v' ''
  let tarball = $"actions-runner-linux-x64-($version).tar.gz"
  let digest = $release.assets | where name == $tarball | first | get digest | str replace 'sha256:' ''
  print $"($tarball) ($digest)"
  { version: $version, sha256: $digest }
}

let codespace = gha group "create codespace" {
  cs api POST $"/repos/($env.REPO)/codespaces" {
    ref: $env.REF
    machine: $machine
    display_name: $label
    idle_timeout_minutes: ($env.IDLE_TIMEOUT | into int)
    retention_period_minutes: 60
    multi_repo_permissions_opt_out: true
  }
}
try {
  gha group "wait for codespace" {
    cs wait "the codespace to become available" 15min { cs get $codespace.name } {|cs|
      if $cs.state in [Failed Deleted Unavailable] {
        error make { msg: $"the codespace is ($cs.state)" }
      }
      $cs.state == "Available"
    } | ignore
  }

  let jit = gha group "register runner" {
    let jit = cs api POST $"/repos/($env.REPO)/actions/runners/generate-jitconfig" {
      name: $label
      runner_group_id: 1
      labels: [self-hosted Linux X64 $label]
      work_folder: _work
    }
    print $"::add-mask::($jit.encoded_jit_config)"
    $jit
  }

  gha group "bootstrap codespace" {
    $jit.encoded_jit_config | cs ssh $codespace.name "umask 077 && mkdir -p /tmp/nrgha && cat > /tmp/nrgha/jit"
    open --raw ($env.FILE_PWD | path join bootstrap.sh)
    | cs ssh $codespace.name $"sudo env RUNNER_VERSION=($release.version) RUNNER_SHA256=($release.sha256) NIX_EXTRA_CONFIG=($env.EXTRA_NIX_CONFIG | encode base64) bash -s"
  }

  gha group "wait for runner" {
    cs wait "the runner to come online" 5min { cs api GET $"/repos/($env.REPO)/actions/runners/($jit.runner.id)" } {|runner|
      $runner.status == "online"
    } | ignore
  }

  # Only publish the outputs once everything works, the workflow routes the build by the result of this job
  $codespace.name | gha output name
  $label | gha output label

  print $"Codespace ($codespace.name) is ready, connect with: gh codespace ssh -c ($codespace.name)"
} catch {|err|
  gha error --title "Failed to start the codespace" $err.msg
  try { cs ssh $codespace.name "sudo tail -n 50 /tmp/nrgha/state/runner.log" }
  exit 1
}
