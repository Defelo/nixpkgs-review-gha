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

  formatter.black = {
    command = lib.getExe pkgs.black;
    includes = [ "*.py" ];
    options = [
      "--line-length=120"
      "--skip-magic-trailing-comma"
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
