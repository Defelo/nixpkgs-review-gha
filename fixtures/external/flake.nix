{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/13043924aaa7375ce482ebe2494338e058282925";
    source = {
      url = "github:Defelo/nixpkgs-review-gha/9d840c2";
      flake = false;
    };
  };
  outputs =
    { nixpkgs, source, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      package = pkgs.runCommand "external-source-fixture" { } ''
        mkdir -p $out/share
        cp ${source}/fixtures/source/message.txt $out/share/message.txt
      '';
    in
    {
      packages.${system}.default = package;
      checks.${system}.smoke = pkgs.runCommand "external-source-check" { } ''
        grep -Fx 'external source fixture' ${package}/share/message.txt
        touch $out
      '';
    };
}
