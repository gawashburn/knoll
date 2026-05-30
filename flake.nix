{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    nixpkgs-mozilla = {
      url = "github:mozilla/nixpkgs-mozilla";
      flake = false;
    };
  };

  outputs = { self, flake-utils, naersk, nixpkgs, nixpkgs-mozilla }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;

          overlays = [
            (import nixpkgs-mozilla)
          ];
        };

        rustChannel = pkgs.rustChannelOf {
          rustToolchain = ./rust-toolchain.toml;
          sha256 = "11c20cr8irfg63j6mj5yl32s5qbs4z4qbfa0ibvrj9nqw480dxcs";
        };

        toolchain = rustChannel.rust.override {
          extensions = [
            "llvm-tools-preview"
            "rust-analyzer-preview"
          ];
        };

        cargo-llvm-cov = pkgs.cargo-llvm-cov.overrideAttrs (old: {
          doCheck = false;
          meta = old.meta // {
            broken = false;
          };
        });

        naersk' = pkgs.callPackage naersk {
          cargo = toolchain;
          rustc = toolchain;
        };

      in rec {
        # For `nix build` & `nix run`:
        defaultPackage = naersk'.buildPackage {
          src = ./.;
        };

        # For `nix develop`:
        devShell = pkgs.mkShell {
          nativeBuildInputs = [
            toolchain
            cargo-llvm-cov
          ];
        };
      }
    );
}
