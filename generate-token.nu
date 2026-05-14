#!/usr/bin/env nu

const github_cli_client_id = "178c6fc778ccc68e1d6a"

def request-code [
  client_id: string
  scopes: string
]: nothing -> record<device_code: string, user_code: string, verification_uri: string, interval: int> {
  let data = {
    client_id: $client_id,
    scope: $scopes,
  }
  http post https://github.com/login/device/code -H { Accept: application/json } --content-type application/json $data
}

def get-token [
  client_id: string
  device_code: string
] {
  let data = {
    client_id: $client_id,
    device_code: $device_code,
    grant_type: "urn:ietf:params:oauth:grant-type:device_code",
  }
  http post https://github.com/login/oauth/access_token -H { Accept: application/json } --content-type application/json $data
}

def main [
  --client-id: string = $github_cli_client_id
  --scopes: string = "public_repo"
] {
  let code_resp = request-code $client_id $scopes
  print -e $"Please enter the code (ansi default_bold)($code_resp.user_code)(ansi reset) on ($code_resp.verification_uri), confirm authorization and then wait up to ($code_resp.interval) seconds"
  loop {
    let token_resp = get-token $client_id $code_resp.device_code
    if $token_resp has access_token {
      print $token_resp.access_token
      break
    } else if $token_resp.error? == authorization_pending {
      sleep ($code_resp.interval * 1sec)
    } else {
      $token_resp | to json | print -e
      exit 1
    }
  }
}
