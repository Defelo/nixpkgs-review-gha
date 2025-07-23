let
  nixpkgs = builtins.getFlake "github:nixos/nixpkgs/nixpkgs-unstable";
  nixpkgs-review = builtins.getFlake "github:Mic92/nixpkgs-review/69fc7fdbcbf06d12198df6d734267dffaf1174ed";
in

import nixpkgs {
  overlays = [ (final: prev: { inherit (nixpkgs-review.packages.${final.system}) nixpkgs-review; }) ];
}
