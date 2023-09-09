{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    cargo2nix = {
      url = "github:cargo2nix/cargo2nix/release-0.11.0";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        rust-overlay.follows = "rust-overlay";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, cargo2nix }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [
            rust-overlay.overlays.default 
            cargo2nix.overlays.default 
          ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          # TODO Why isn't this exposed in pkgs by the overlays?
          cargo2nix_pkg = 
            cargo2nix.packages."${system}".cargo2nix;

          rustVersion = "1.71.1";
          rustPkgs = pkgs.rustBuilder.makePackageSet {
            inherit rustVersion;
            packageFun = import ./Cargo.nix;
            packageOverrides = pkgs: pkgs.rustBuilder.overrides.all;
          };

          workspaceShell = rustPkgs.workspaceShell {
            packages = [
              cargo2nix_pkg
              # TODO Currently cannot pull in all needed private frameworks.
              #pkgs.darwin.apple_sdk.frameworks.*?
            ];
          };
        in rec {
          # TODO Fix reported warnings.
          devShells.default = workspaceShell;
          packages = {
            knoll = rustPkgs.workspace.knoll {};
            default = packages.knoll;
          };
        }
      );
}
