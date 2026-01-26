{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    { self, nixpkgs }:

    let
      inherit (nixpkgs) lib;

      eachSystem = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
    in

    {
      packages = eachSystem (pkgs: {
        generate-markdown-report = pkgs.writers.writePython3Bin "generate-markdown-report" {
          flakeIgnore = [
            "E501" # line too long
          ];
        } (builtins.readFile ./generate_markdown_report.py);
      });

      legacyPackages = eachSystem lib.id;

      formatter = eachSystem (
        pkgs:
        pkgs.treefmt.withConfig {
          settings = lib.mkMerge [
            ./treefmt.nix
            { _module.args = { inherit pkgs; }; }
          ];
        }
      );

      checks = eachSystem (pkgs: {
        inherit (pkgs) nixpkgs-review;
        packages = pkgs.linkFarm "packages" self.packages.${pkgs.stdenv.hostPlatform.system};
        fmt = pkgs.runCommand "fmt-check" { } ''
          cp -r --no-preserve=mode ${self} repo
          ${lib.getExe self.formatter.${pkgs.stdenv.hostPlatform.system}} -C repo --ci
          touch $out
        '';
      });
    };

  nixConfig = {
    abort-on-warn = true;
    commit-lock-file-summary = "chore: update flake.lock";
  };
}
