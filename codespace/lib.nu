# Helpers for talking to the Codespaces and Actions REST APIs through `gh`.
# All commands authenticate with $env.GH_TOKEN, which must hold the CODESPACE_TOKEN secret.

# Call the GitHub REST API and parse the JSON response.  The optional body is sent as JSON.
export def "cs api" [method: string, path: string, body?: record] {
  if $body == null {
    ^gh api -X $method $path
  } else {
    $body | to json -r | ^gh api -X $method $path --input -
  } | from json
}

# Name shared by the codespace and the runner of the current workflow run attempt
export def "cs label" [] {
  $"nrgha-($env.RUN_ID)-($env.RUN_ATTEMPT)"
}

# Call the GitHub REST API and ignore the response, tolerating failures such as 404 on cleanup.
export def "cs api try" [method: string, path: string] {
  ^gh api -X $method $path | complete | ignore
}

# Run a shell command line inside the codespace.  Anything piped in is forwarded to the remote standard input.
export def "cs ssh" [name: string, command: string] {
  ^gh codespace ssh -c $name -- $command
}

# Fetch the codespace record, e.g. to look at its state.
export def "cs get" [name: string] {
  cs api GET $"/user/codespaces/($name)"
}

# Poll until the predicate holds for the value returned by the getter, failing after the timeout.
# Errors of the getter are treated as a missing value, because the API fails transiently now and then.
export def "cs wait" [
  what: string
  timeout: duration
  getter: closure
  predicate: closure
] {
  let deadline = (date now) + $timeout
  loop {
    let value = try { do $getter } catch { null }
    if $value != null and (do $predicate $value) { return $value }
    if (date now) > $deadline {
      error make { msg: $"timed out after ($timeout) waiting for ($what)" }
    }
    sleep 5sec
  }
}
