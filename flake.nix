{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    simit = {
      url = "github:caniko/simit/fb0d92f12226e4191b94e9c026f5852f913f3db4";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      simit,
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      importNixpkgs =
        system:
        import nixpkgs {
          inherit system;
          config.allowDeprecatedx86_64Darwin = true;
        };
      eachSystem = f: lib.genAttrs systems (system: f (importNixpkgs system));
      tools =
        system:
        import (simit + "/nix/review-tools.nix") {
          source = simit;
          inherit system nixpkgs;
        };
    in
    {
      legacyPackages = eachSystem lib.id;
      packages = eachSystem (
        pkgs:
        let
          engine = tools pkgs.stdenv.hostPlatform.system;
        in
        {
          repo-review = engine.package;
          default = engine.package;
          nixpkgs-selector = engine.selector;
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
        nixpkgs-selector = (tools pkgs.stdenv.hostPlatform.system).selectorCheck;
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
