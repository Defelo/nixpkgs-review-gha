{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    { self, nixpkgs, ... }:

    let
      inherit (nixpkgs) lib;

      eachSystem = f: lib.genAttrs systems (s: f nixpkgs.legacyPackages.${s});
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
    in

    {
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
        fmt = pkgs.runCommandNoCCLocal "fmt-check" { } ''
          cp -r --no-preserve=mode ${./.} repo
          ${lib.getExe self.formatter.${pkgs.system}} -C repo --ci --tree-root-file=treefmt.nix
          touch $out
        '';
      });
    };
}
