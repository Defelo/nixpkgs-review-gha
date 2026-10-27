{ lib, pkgs, ... }:

{
  tree-root-file = "treefmt.nix";
  on-unmatched = "fatal";

  excludes = [
    "*.lock"
    "*.md"
    "*.nu"
    "*.patch"
    "*.snap"
    "*/.gitignore"
    ".gitignore"
    "LICENSE"
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
    ];
    options = [
      "--write"
      "--print-width=120"
      "--arrow-parens=avoid"
    ];
  };

  formatter.shfmt = {
    command = lib.getExe pkgs.shfmt;
    includes = [ "*.sh" ];
    options = [
      "--write"
      "--indent=2"
    ];
  };

  formatter.rustfmt = {
    command = lib.getExe pkgs.rustfmt;
    includes = [ "*.rs" ];
    options = [
      "--config=skip_children=true"
      "--edition=2024"
    ];
  };

  formatter.taplo = {
    command = lib.getExe pkgs.taplo;
    includes = [ "*.toml" ];
    options = [
      "format"
      "--option=column_width=120"
      "--option=align_comments=false"
    ];
  };
}
