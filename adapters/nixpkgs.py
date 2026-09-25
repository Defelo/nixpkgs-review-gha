"""Pinned nixpkgs-review 3.7.0 API adapter; never resolves a PR/ref itself.

Selection uses upstream Review.build_commit/list_packages/differences/nix_eval.
Only the realization boundary is replaced so Rust can freeze the derivations
before building. No custom package change detector is maintained here.
"""
import json
import os
from importlib.metadata import version
from pathlib import Path
import subprocess
import sys

from nixpkgs_review.allow import AllowedFeatures
from nixpkgs_review.builddir import Builddir
from nixpkgs_review.buildenv import Buildenv
from nixpkgs_review.nix import nix_eval
from nixpkgs_review.review import CheckoutOption, Review


class SelectOnly(Review):
    def build(self, packages_per_system, args):
        if set(packages_per_system) != {selected_system}:
            raise RuntimeError("changed-package detector returned unexpected systems")
        selected = sorted(packages_per_system[selected_system])
        attrs = nix_eval(set(selected), selected_system, self.allow, self.builddir.nix_path)
        if any(a.broken or a.blacklisted or not a.exists or not a.drv_path for a in attrs):
            raise RuntimeError("selected package unavailable, broken, or blacklisted")
        actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.worktree_dir(), text=True).strip()
        if actual != plan["target"]["commit"]:
            raise RuntimeError("nixpkgs-review checkout differs from frozen tested commit")
        output.write_text(json.dumps({
            "schema_version": 1,
            "backend_version": version("nixpkgs-review"),
            "tested_commit": actual,
            "base_commit": plan["pr"]["base"],
            "system": selected_system,
            "changed_attributes": selected,
            "derivations": [{"attribute": a.name, "aliases": a.aliases, "derivation": a.drv_path, "check": a.is_test()} for a in attrs],
        }, sort_keys=True, indent=2) + "\n")
        return {selected_system: attrs}


if version("nixpkgs-review") != "3.7.0":
    raise RuntimeError("unsupported nixpkgs-review API version; review adapter before updating tool lock")
plan = json.loads(Path(sys.argv[1]).read_text())
selected_system = sys.argv[2]
output = Path(sys.argv[3])
# Fixed administrator-owned limits for upstream subprocesses, not request config.
os.environ["NIX_CONFIG"] = "accept-flake-config = false\nallow-import-from-derivation = false\nbuilders =\nmax-jobs = 2\ncores = 2\ntimeout = 1800\nmax-silent-time = 600\n"
allow = AllowedFeatures([])
with Buildenv(False, "{ }") as config, Builddir("exact-" + plan["digest"]) as builddir:
    review = SelectOnly(builddir=builddir, build_args="", no_shell=True, run="", remote="",
                        systems=[selected_system], allow=allow, build_graph="nix",
                        nixpkgs_config=config, extra_nixpkgs_config="{ }", eval_type="local",
                        checkout=CheckoutOption.COMMIT, num_parallel_evals=1)
    # Passing the tested SHA in merge_commit bypasses git_merge in upstream.
    # Use that same SHA as head_commit so COMMIT cannot switch to another tree.
    review.build_commit(plan["pr"]["base"], plan["target"]["commit"], plan["target"]["commit"])
