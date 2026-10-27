use ../review/gha.nu *
use lib.nu *

# Keep the codespace awake while the runner works on its job.
# GitHub stops idle codespaces, and only connected clients count as activity, so this job holds an SSH
# session open.  A job can run for at most 6 hours, so the output "continue" tells the workflow whether
# another guard job has to take over once the time budget is spent.

let name = $env.NAME
let deadline = (date now) + ($env.BUDGET_MINUTES | into int | into duration --unit min)
let watcher = open --raw ($env.FILE_PWD | path join watch.sh)

# Answer null instead of failing on transient API errors
def codespace-info [name: string] {
  try { cs get $name } catch { null }
}

# The remote command always succeeds, otherwise gh complains about the exit status of a busy runner
def runner-finished [name: string] {
  try { cs ssh $name "test -e /tmp/nrgha/state/runner.exit && echo finished || true" | str trim | $in == "finished" } catch { false }
}

let created = (codespace-info $name).created_at?
let limit = if $created == null { null } else { ($created | into datetime) + 12hr }
if $limit != null {
  print $"GitHub stops codespaces after 12 hours, so this one will be stopped at ($limit | date to-timezone utc | format date '%H:%M UTC')"
}

mut last_healthy = date now
loop {
  let remaining = ($deadline - (date now)) / 1min | math ceil

  if $remaining <= 0 {
    print "Time budget spent, handing over to the next guard job"
    "true" | gha output continue
    break
  }

  let state = (codespace-info $name).state?
  if $state != null and $state != "Available" {
    gha error --title "Codespace stopped" $"The codespace is ($state), so the build cannot continue. Codespaces are stopped by GitHub after 12 hours or when they run out of quota."
    exit 1
  }

  let started = date now
  let attached = try {
    let limit_arg = if $limit == null { "" } else { $limit | format date '%s' }
    $watcher | ^gh codespace ssh -c $name -- $"bash -s -- ($remaining) ($limit_arg)"
    true
  } catch { false }

  if (runner-finished $name) {
    "false" | gha output continue
    break
  }

  if $attached or (date now) - $started > 1min {
    $last_healthy = date now
  } else if (date now) - $last_healthy > 20min {
    gha error --title "Lost the codespace" "Could not keep an SSH session open to the codespace for 20 minutes"
    exit 1
  } else {
    print "SSH session ended early, reconnecting"
    sleep 15sec
  }
}
