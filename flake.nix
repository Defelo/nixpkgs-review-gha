{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    { self, nixpkgs }:

    let
      inherit (nixpkgs) lib;

      importNixpkgs =
        system:
        import nixpkgs {
          inherit system;
          config.allowDeprecatedx86_64Darwin = true;
        };

      eachSystem = f: lib.genAttrs systems (system: f (importNixpkgs system));
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
        "riscv64-linux"
      ];
    in

    {
      packages = eachSystem (pkgs: {
        generate-token = pkgs.writers.writeNuBin "generate-token" (builtins.readFile ./generate-token.nu);
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
