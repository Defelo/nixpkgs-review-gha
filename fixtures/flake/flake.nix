{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/13043924aaa7375ce482ebe2494338e058282925";
  outputs =
    { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      package = pkgs.writeShellScriptBin "review-fixture" ''
        exec ${pkgs.coreutils}/bin/printf '%s\n' 'repo-review fixture'
      '';
    in
    {
      packages.${system}.default = package;
      checks.${system} = {
        smoke = pkgs.runCommand "review-fixture-check" { } ''
          test "$(${package}/bin/review-fixture)" = 'repo-review fixture'
          touch $out
        '';
        failing = pkgs.runCommand "review-fixture-failure" { } ''
          echo 'intentional failing check' >&2
          exit 1
        '';
      };
    };
}
