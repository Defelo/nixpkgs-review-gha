"""CI-only compatibility boundary regression using the packaged request validator."""
import json
from pathlib import Path
import subprocess
import tempfile

cli = str(Path("tools/bin/repo-review").resolve())
request = json.loads(subprocess.check_output([cli, "example"], text=True))
with tempfile.TemporaryDirectory() as temporary:
    path = Path(temporary) / "request.json"
    for publication in ("none", "request-approval"):
        candidate = dict(request, publication=publication)
        if publication == "request-approval":
            candidate["cache_profile"] = "attic-existing"
        normalized = subprocess.check_output([cli, "validate", "-"], input=json.dumps(candidate), text=True)
        path.write_text(normalized)
        result = subprocess.run(["python3", "compatibility-request.py", str(path)], capture_output=True, text=True)
        if publication == "none":
            assert result.returncode == 0, result.stderr
        else:
            assert result.returncode != 0 and "review-repository.yml directly" in result.stderr
