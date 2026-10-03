"""CI-only Nix acceptance. Uses exact controller commit, no publication credentials."""
import json
import os
from pathlib import Path
import subprocess

cli = str(Path("tools/bin/repo-review").resolve())
repo = os.environ["GITHUB_REPOSITORY"]
revision = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
base = json.loads(subprocess.check_output([cli, "example"], text=True))
base.update(repository=repo, pr=None, revision=revision, test_profile="checks-rebuild-v1")
evidence = Path("fixture-evidence")
evidence.mkdir()
for kind in ("flake", "external-flake", "failing"):
    request = dict(base)
    request["directory"] = "fixtures/flake"
    if kind == "external-flake":
        recipe = json.loads(Path("policy.json").read_text())["recipes"][0]
        request.update(backend=kind, directory=".", recipe=recipe)
    if kind == "failing":
        request["checks"] = ["failing"]
    path = evidence / f"{kind}-request.json"
    path.write_text(json.dumps(request))
    plan = evidence / f"{kind}-plan.json"
    subprocess.run([cli, "plan", str(path), "--controller", repo, "--revision", revision, "--output", str(plan)], check=True)
    env = dict(os.environ)
    env.pop("GH_TOKEN", None)
    env.pop("GITHUB_TOKEN", None)
    bundle = evidence / kind
    result = subprocess.run([cli, "build", "--plan", str(plan), "--system", "x86_64-linux", "--output", str(bundle.resolve())], env=env)
    report = json.loads((bundle / "review-result.json").read_text())
    if kind == "failing":
        assert result.returncode != 0 and report["build"] == "failed" and report["tests"] == "failed"
    else:
        assert result.returncode == 0, report.get("error")
        subprocess.run([cli, "validate-report", "--plan", str(plan), "--bundle", str(bundle)], check=True, env=env)
        subprocess.run([cli, "verify-local", "--plan", str(plan), "--bundle", str(bundle), "--destination", str((evidence / f"{kind}-fresh-store").resolve())], check=True, env=env)
