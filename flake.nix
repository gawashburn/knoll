{
  inputs = {
    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, crane, fenix, flake-utils, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        fenixPkgs = fenix.packages.${system};

        stableToolchain = fenixPkgs.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "11c20cr8irfg63j6mj5yl32s5qbs4z4qbfa0ibvrj9nqw480dxcs";
        };

        nightlyToolchain = fenixPkgs.latest.withComponents [
          "cargo"
          "llvm-tools-preview"
          "rustc"
        ];

        cargo-llvm-cov = pkgs.cargo-llvm-cov.overrideAttrs (old: {
          doCheck = false;
          meta = old.meta // {
            broken = false;
          };
        });

        craneLibBase = (crane.mkLib pkgs).overrideScope (_final: _prev: {
          inherit cargo-llvm-cov;
        });
        craneLib = craneLibBase.overrideToolchain stableToolchain;
        craneLibNightly = craneLibBase.overrideToolchain nightlyToolchain;

        src = craneLib.cleanCargoSource ./.;
        commonArgs = {
          inherit src;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        nightlyCargoArtifacts = craneLibNightly.buildDepsOnly commonArgs;

        knoll = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

        coverage = craneLibNightly.cargoLlvmCov (commonArgs // {
          cargoArtifacts = nightlyCargoArtifacts;
          cargoExtraArgs = "--locked --all-features --no-fail-fast";
          cargoLlvmCovExtraArgs = "--branch --lcov --output-path $out";
        });
      in {
        checks = {
          inherit knoll coverage;
        };

        packages = {
          default = knoll;
          inherit coverage;
        };

        apps.default = {
          type = "app";
          program = "${knoll}/bin/knoll";
          meta.description = "Configure macOS displays";
        };

        devShells = {
          default = craneLib.devShell {
            checks = {
              inherit knoll;
            };
          };

          coverage = craneLibNightly.devShell {
            packages = [
              cargo-llvm-cov
            ];
          };
        };
      }
    );
}
