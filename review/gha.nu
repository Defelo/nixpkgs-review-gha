export def "gha group" [name: string, block: closure] {
  print $"::group::($name)"
  let result = do $block
  print "::endgroup::"
  $result
}

def "gha log" [level: string, args: record, msg: string] {
  $args
  | compact
  | transpose name value
  | each { $"($in.name)=($in.value)" }
  | str join ","
  | print $"::($level) ($in)::($msg)"
}

export def "gha warning" [--title: string, msg: string] {
  gha log warning { title: $title } $msg
}

export def "gha error" [--title: string, msg: string] {
  gha log error { title: $title } $msg
}

export def "gha output" [name: string] {
  $"($name)=($in)\n" | save -ra $env.GITHUB_OUTPUT
}

export def "gha step-summary" [] {
  save -rf $env.GITHUB_STEP_SUMMARY
}

export def "gha get-oidc-token" [audience: string] {
  let headers = { Authorization: $"bearer ($env.ACTIONS_ID_TOKEN_REQUEST_TOKEN)" }
  $env.ACTIONS_ID_TOKEN_REQUEST_URL
  | url parse
  | reject query
  | update params { append { key: audience, value: $audience } }
  | url join
  | http get -H $headers $in
  | get value
}

export def "gha review-inputs" [] {
  $env.INPUTS
  | from json
  | upsert extra-args { default "" }
  | upsert extra-build-args { default "" }
  | update cells -c [
    x86_64-linux
    aarch64-linux
    riscv64-linux
    push-to-cache
    upterm
    post-result
  ] { $in == "true" }
  | update pr { into int }
  | insert extra-args-raw { $in.extra-args }
  | update extra-args { $"[($in)]" | from nuon }
}
