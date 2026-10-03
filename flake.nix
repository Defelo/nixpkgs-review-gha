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
      ];
    in

    {
      legacyPackages = eachSystem lib.id;

      packages = eachSystem (
        pkgs:
        let
          reviewPython = pkgs.python3.withPackages (ps: [ (ps.toPythonModule pkgs.nixpkgs-review) ]);
          selector = pkgs.writeShellScriptBin "repo-review-nixpkgs-select" ''
            exec ${reviewPython}/bin/python ${./adapters/nixpkgs.py} "$@"
          '';
        in
        rec {
          repo-review = pkgs.rustPlatform.buildRustPackage {
            pname = "repo-review";
            version = "0.1.0";
            src = lib.cleanSource self;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            postInstall = ''
              wrapProgram $out/bin/repo-review --prefix PATH : ${
                lib.makeBinPath [
                  pkgs.git
                  pkgs.nix
                  pkgs.gh
                  pkgs.nixpkgs-review
                  pkgs.attic-client
                  pkgs.cachix
                  pkgs.coreutils
                  selector
                ]
              }
            '';
            meta.mainProgram = "repo-review";
          };
          default = repo-review;
          nixpkgs-selector = selector;
        }
      );

      apps = eachSystem (pkgs: {
        repo-review = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.repo-review}/bin/repo-review";
        };
      });

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
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) repo-review;
        nixpkgs-selector = pkgs.runCommand "nixpkgs-selector-import-check" { } ''
          ${
            self.packages.${pkgs.stdenv.hostPlatform.system}.nixpkgs-selector
          }/bin/repo-review-nixpkgs-select --self-test > $out
        '';
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
