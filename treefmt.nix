{ lib, pkgs, ... }:

{
  tree-root-file = "treefmt.nix";
  on-unmatched = "fatal";

  excludes = [
    "*.lock"
    "*.md"
    ".gitignore"
    "LICENSE"
  ];

  formatter.actionlint = {
    command = lib.getExe pkgs.actionlint;
    includes = [
      ".github/workflows/*.yml"
      ".github/workflows/*.yaml"
    ];
    options = [
      "-verbose"
      "-shellcheck"
      (lib.getExe pkgs.shellcheck)
    ];
  };

  formatter.nixfmt = {
    command = lib.getExe pkgs.nixfmt;
    includes = [ "*.nix" ];
    options = [ "--strict" ];
  };

  formatter.prettier = {
    command = lib.getExe pkgs.nodePackages.prettier;
    includes = [
      "*.js"
      "*.yml"
    ];
    options = [
      "--write"
      "--print-width=120"
      "--arrow-parens=avoid"
    ];
  };
}
