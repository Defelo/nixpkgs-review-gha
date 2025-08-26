{ lib, pkgs, ... }:

{
  tree-root-file = ".git/config";
  on-unmatched = "fatal";

  excludes = [
    "*.lock"
    "*.md"
    ".gitignore"
    "LICENSE"
  ];

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
