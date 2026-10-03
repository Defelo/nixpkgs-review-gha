"""CI-only: real pinned Review.build_commit/differences + adapter, recorded Nix I/O."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
from unittest.mock import patch

from nixpkgs_review.allow import AllowedFeatures
from nixpkgs_review.nix import Attr
import nixpkgs_review.review as upstream

spec = importlib.util.spec_from_file_location("adapter", sys.argv[1])
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)
record = json.loads(Path(sys.argv[2]).read_text())
system = "x86_64-linux"

def packages(rows):
    return [upstream.Package(pname=r["attribute"], version=r["version"], attr_path=r["attribute"], store_path=r["path"], homepage=None, description=None, position=None) for r in rows]

with tempfile.TemporaryDirectory() as temp:
    for tested in (record["head"], record["merge"]):
        for changed in (True, False):
            plan = {"pr": {"base": record["base"]}, "target": {"commit": tested}}
            current = [None]
            output = Path(temp) / "selection.json"
            old, new = packages(record["old"]), packages(record["new"] if changed else record["old"])
            def list_packages(nix_path, systems, allow, n_threads, check_meta=False):
                assert systems == {system}
                assert current[0] in (record["base"], tested)
                return {system: new if check_meta else old}
            def nix_eval(names, native, allow, nix_path):
                assert native == system and current[0] == tested
                assert names == ({"hello", "nixosTests.smoke"} if changed else set())
                return [Attr(name=n, exists=True, broken=False, blacklisted=False, path=Path(r["path"]), drv_path=r["path"] + ".drv") for r in record["new"] if (n := r["attribute"]) in names]
            with patch.object(upstream, "current_system", return_value=system), \
                 patch.object(upstream, "list_packages", side_effect=list_packages), \
                 patch.object(adapter, "nix_eval", side_effect=nix_eval), \
                 patch.object(adapter.subprocess, "check_output", side_effect=lambda *a, **kw: current[0]), \
                 patch.object(upstream.Review, "git_worktree", side_effect=lambda sha: current.__setitem__(0, sha)), \
                 patch.object(upstream.Review, "git_checkout", side_effect=lambda sha: current.__setitem__(0, sha)), \
                 patch.object(upstream.Review, "git_merge", side_effect=AssertionError("must not synthesize a merge")):
                review = adapter.SelectOnly(plan, system, output, builddir=SimpleNamespace(nix_path="recorded", worktree_dir=Path(temp)), build_args="", no_shell=True, run="", remote="", systems=[system], allow=AllowedFeatures([]), build_graph="nix", nixpkgs_config=Path(temp), extra_nixpkgs_config="{}", eval_type="local", checkout=upstream.CheckoutOption.COMMIT)
                review.build_commit(record["base"], tested, tested)
            result = json.loads(output.read_text())
            assert result["tested_commit"] == tested and result["base_commit"] == record["base"]
            assert result["changed_attributes"] == (["hello", "nixosTests.smoke"] if changed else [])
            assert len(result["derivations"]) == (2 if changed else 0)
            if changed:
                assert [d["check"] for d in result["derivations"]] == [False, True]
print("Recorded Nixpkgs head, merge and no-change selection: passed")
