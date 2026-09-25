{ lib, pkgs, ... }:

{
  tree-root-file = "treefmt.nix";
  on-unmatched = "fatal";

  excludes = [
    "*.lock"
    "*.md"
    ".gitignore"
    "LICENSE"
    ".envrc"
    "*.txt"
    "*.py"
    "schemas/*.json"
    "examples/request.json"
  ];

  formatter.nixfmt = {
    command = lib.getExe pkgs.nixfmt;
    includes = [ "*.nix" ];
    options = [ "--strict" ];
  };

  formatter.prettier = {
    command = lib.getExe pkgs.prettier;
    includes = [
      "*.js"
      "*.yml"
      "*.json"
    ];
    options = [
      "--write"
      "--print-width=120"
      "--arrow-parens=avoid"
    ];
  };
  formatter.rustfmt = {
    command = lib.getExe pkgs.rustfmt;
    includes = [ "*.rs" ];
    options = [
      "--edition"
      "2024"
    ];
  };
  formatter.taplo = {
    command = lib.getExe pkgs.taplo;
    includes = [ "*.toml" ];
    options = [ "fmt" ];
  };
}
