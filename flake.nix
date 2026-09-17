{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    { self, nixpkgs, ... }@inputs:

    let
      inherit (nixpkgs) lib;

      importNixpkgs = system: nixpkgs.legacyPackages.${system};

      eachSystem = f: lib.genAttrs systems (system: f (importNixpkgs system));
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "riscv64-linux"
      ];
    in

    {
      packages = eachSystem (pkgs: {
        nrgha-api = pkgs.callPackage ./api/package.nix { inherit (inputs) crane; };
      });

      nixosModules.nrgha-api = import ./api/module.nix { inherit self; };

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
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}.nrgha-api) clippy;
        packages = pkgs.linkFarm "packages" self.packages.${pkgs.stdenv.hostPlatform.system};
        fmt = pkgs.runCommand "fmt-check" { } ''
          cp -r --no-preserve=mode ${self} repo
          ${lib.getExe self.formatter.${pkgs.stdenv.hostPlatform.system}} -C repo --ci
          touch $out
        '';
        nu = pkgs.runCommand "nu-check" { } ''
          ${lib.getExe pkgs.nushell} -c 'for x in (glob ${self}/**/*.nu) { print $"checking ($x)"; nu-check $x -d }'
          touch $out
        '';
        actionlint =
          pkgs.runCommand "actionlint"
            {
              nativeBuildInputs = [
                pkgs.actionlint
                pkgs.shellcheck
              ];
            }
            ''
              cp -r --no-preserve=mode ${self} src
              cd src
              actionlint
              touch $out
            '';
      });
    };

  nixConfig = {
    abort-on-warn = true;
    commit-lock-file-summary = "flake.lock: update";
  };
}
