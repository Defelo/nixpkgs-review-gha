let
  nixpkgs = builtins.getFlake "github:nixos/nixpkgs/nixpkgs-unstable";
in

import nixpkgs { overlays = [ (import ./nixpkgs-review/overlay.nix) ]; }
