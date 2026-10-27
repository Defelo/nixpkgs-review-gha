use ../review/gha.nu *
use lib.nu *

let label = cs label

gha group "remove runner" {
  cs api GET $"/repos/($env.REPO)/actions/runners?per_page=100"
  | get runners
  | where name == $label
  | each { cs api try DELETE $"/repos/($env.REPO)/actions/runners/($in.id)" }
  | ignore
}

gha group "delete codespace" {
  cs api GET $"/repos/($env.REPO)/codespaces?per_page=100"
  | get codespaces
  | where display_name == $label
  | each { cs api try DELETE $"/user/codespaces/($in.name)" }
  | ignore
}
