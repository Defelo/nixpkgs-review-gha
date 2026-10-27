#!/usr/bin/env bash
# Prepare a GitHub Codespace to run a single ephemeral GitHub Actions runner job.
#
# This script runs as root inside the codespace.  The environment provides:
#   RUNNER_VERSION    actions/runner release to install, for example 2.338.1
#   RUNNER_SHA256     expected SHA-256 digest of the release tarball
#   NIX_EXTRA_CONFIG  base64 encoded nix.conf lines to append (may be empty)
# The just-in-time runner configuration is read from /tmp/nrgha/jit.
#
# Everything heavy lives under /tmp because codespaces mount a separate, larger disk there.
set -euo pipefail

base=/tmp/nrgha
state=$base/state
runner_dir=$base/runner

group() { echo "::group::$1"; }
endgroup() { echo "::endgroup::"; }

group "create runner user"
install -d -m 0755 "$base"
# Codespaces puts a default ACL on /tmp that makes everything created there world-writable, and Nix
# refuses to use a world-writable build directory.  Dropping the ACL keeps it out of our subtree.
python3 -c 'import os, sys; os.removexattr(sys.argv[1], "system.posix_acl_default")' "$base" 2>/dev/null ||
  setfacl -k "$base" 2>/dev/null || true
# Unprivileged user without sudo, because the runner executes untrusted pull request code
id runner >/dev/null 2>&1 || useradd --create-home --home-dir "$base/home" --shell /bin/bash runner
chmod 0700 "$base/home"
install -d -o runner -g runner -m 0755 "$state" "$runner_dir"
endgroup

group "hide codespace secrets from the runner"
# Codespaces leaves its environment and user secrets readable by every user in the container
[ -d /workspaces/.codespaces ] && chmod 0700 /workspaces/.codespaces
for secrets in /workspaces/.codespaces/shared/.env-secrets /workspaces/.codespaces/shared/.user-secrets.json; do
  if [ -e "$secrets" ] && sudo -u runner test -r "$secrets"; then
    echo "::error::$secrets is readable by the runner user"
    exit 1
  fi
done
endgroup

group "mount /nix on the large disk"
install -d -m 0755 /nix "$base/nix"
mountpoint -q /nix || mount --bind "$base/nix" /nix
df -h / /tmp /nix
endgroup

group "install nix"
conf=$(mktemp)
{
  cat <<'EOF'
accept-flake-config = true
auto-allocate-uids = true
build-dir = /nix/build
builders =
builders-use-substitutes = true
extra-experimental-features = auto-allocate-uids cgroups flakes nix-command
extra-system-features = uid-range
sandbox = true
system = x86_64-linux
EOF
  printf '%s' "${NIX_EXTRA_CONFIG:-}" | base64 -d
  echo
} >"$conf"
curl -fsSL https://nixos.org/nix/install -o "$base/nix-install"
sh "$base/nix-install" --daemon --yes --no-channel-add --nix-extra-conf-file "$conf"
mkdir -p /nix/build
endgroup

group "start nix-daemon"
# No init system in the container, so a small loop keeps the daemon running
# shellcheck disable=SC1091
. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
setsid nohup bash -c 'while true; do nix-daemon --daemon; sleep 1; done' >"$base/nix-daemon.log" 2>&1 </dev/null &
for _ in $(seq 1 30); do
  [ -S /nix/var/nix/daemon-socket/socket ] && break
  sleep 1
done
sudo -u runner -H bash -c '. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh && nix --version && nix config show system'
endgroup

group "install actions runner"
curl -fsSL -o "$base/runner.tar.gz" \
  "https://github.com/actions/runner/releases/download/v$RUNNER_VERSION/actions-runner-linux-x64-$RUNNER_VERSION.tar.gz"
echo "$RUNNER_SHA256  $base/runner.tar.gz" | sha256sum -c -
tar xzf "$base/runner.tar.gz" -C "$runner_dir"
chown -R runner:runner "$runner_dir"
# Skip the slow apt run when the default codespace image already ships the runtime libraries
sudo -u runner "$runner_dir/bin/Runner.Listener" --version || "$runner_dir/bin/installdependencies.sh"
endgroup

group "start runner"
chown runner:runner "$base/jit"
chmod 0600 "$base/jit"
cat >"$base/run-runner" <<'EOF'
#!/usr/bin/env bash
. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
jit="$(cat /tmp/nrgha/jit)"
rm -f /tmp/nrgha/jit
cd /tmp/nrgha/runner
./run.sh --jitconfig "$jit" >/tmp/nrgha/state/runner.log 2>&1
echo $? >/tmp/nrgha/state/runner.exit
EOF
chmod 0755 "$base/run-runner"
setsid nohup sudo -u runner -H bash "$base/run-runner" >/dev/null 2>&1 </dev/null &
sleep 5
pgrep -u runner -f Runner.Listener >/dev/null || {
  echo "::error::the runner process is not running"
  cat "$state/runner.log" || true
  exit 1
}
endgroup
