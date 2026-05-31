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
    let
      mkKnollOptions =
        { lib, pkgs, defaultLogDirectory }:
        let
          inherit (lib) mkOption types;
          absolutePath = types.strMatching "/.*";
          json = pkgs.formats.json { };
          displayConfigType = types.attrsOf json.type;
        in
        {
          enable = lib.mkEnableOption "knoll display configuration daemon";

          package = mkOption {
            type = types.package;
            inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) default;
            description = "The knoll package to run.";
          };

          displayConfigs = mkOption {
            type = types.listOf (types.listOf displayConfigType);
            example = lib.literalExpression ''
              [
                [
                  {
                    uuid = "8684ad81e3ea92cb14f43eb88b97a3f7";
                    enabled = true;
                    origin = [ (-1792) 453 ];
                    extents = [ 1792 1120 ];
                    scaled = true;
                    frequency = 59;
                    color_depth = 8;
                    rotation = 0;
                  }
                ]
              ]
            '';
            description = ''
              Display configuration groups passed to knoll as JSON.
            '';
          };

          stdoutPath = mkOption {
            type = types.nullOr absolutePath;
            default =
              if defaultLogDirectory == null then null else "${defaultLogDirectory}/knoll.out.log";
            description = ''
              Absolute path for knoll daemon stdout. Set to null to let launchd handle stdout.
            '';
          };

          stderrPath = mkOption {
            type = types.nullOr absolutePath;
            default =
              if defaultLogDirectory == null then null else "${defaultLogDirectory}/knoll.err.log";
            description = ''
              Absolute path for knoll daemon stderr. Set to null to let launchd handle stderr.
            '';
          };

          verbosity = mkOption {
            type = types.ints.between 0 4;
            default = 1;
            description = ''
              Verbosity to pass to knoll daemon. 0 logs errors, 1 warnings, 2 info, 3 debug, and 4 trace.
            '';
          };
        };

      mkProgramArguments =
        { cfg, lib, pkgs }:
        let
          configFile = pkgs.writeText "knoll-config.json" (builtins.toJSON cfg.displayConfigs);
        in
        [
          "${cfg.package}/bin/knoll"
          "daemon"
        ] ++ lib.replicate cfg.verbosity "-v" ++ [
          "--format=json"
          "--input=${configFile}"
        ];

      darwinModule =
        { config, lib, pkgs, ... }:
        let
          cfg = config.services.knoll;
          defaultLogDirectory =
            if config.system.primaryUser == null then
              null
            else
              "/Users/${config.system.primaryUser}/Library/Logs";
        in
        {
          options.services.knoll = mkKnollOptions { inherit lib pkgs defaultLogDirectory; };

          config = lib.mkIf cfg.enable {
            assertions = [
              {
                assertion = pkgs.stdenv.hostPlatform.isDarwin;
                message = "services.knoll is only supported on Darwin.";
              }
              {
                assertion = config.system.primaryUser != null;
                message = "services.knoll requires system.primaryUser because it installs a user LaunchAgent.";
              }
            ];

            launchd.user.agents.knoll = {
              serviceConfig = {
                Label = "local.knoll";
                ProgramArguments = mkProgramArguments { inherit cfg lib pkgs; };
                KeepAlive = true;
                RunAtLoad = true;
              } // lib.optionalAttrs (cfg.stderrPath != null) {
                StandardErrorPath = cfg.stderrPath;
              } // lib.optionalAttrs (cfg.stdoutPath != null) {
                StandardOutPath = cfg.stdoutPath;
              };
            };
          };
        };

      homeModule =
        { config, lib, pkgs, ... }:
        let
          cfg = config.services.knoll;
          defaultLogDirectory = "${config.home.homeDirectory}/Library/Logs";
        in
        {
          options.services.knoll = mkKnollOptions { inherit lib pkgs defaultLogDirectory; };

          config = lib.mkIf cfg.enable {
            assertions = [
              {
                assertion = pkgs.stdenv.hostPlatform.isDarwin;
                message = "services.knoll is only supported on Darwin.";
              }
            ];

            launchd.agents.knoll = {
              enable = true;
              config = {
                Label = "local.knoll";
                ProgramArguments = mkProgramArguments { inherit cfg lib pkgs; };
                KeepAlive = true;
                RunAtLoad = true;
              } // lib.optionalAttrs (cfg.stderrPath != null) {
                StandardErrorPath = cfg.stderrPath;
              } // lib.optionalAttrs (cfg.stdoutPath != null) {
                StandardOutPath = cfg.stdoutPath;
              };
            };
          };
        };
    in
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        fenixPkgs = fenix.packages.${system};

        # Normal builds use the checked-in Rust pin so Cargo, rustc, and
        # rust-analyzer stay aligned across local development and Nix builds.
        stableToolchain = fenixPkgs.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "11c20cr8irfg63j6mj5yl32s5qbs4z4qbfa0ibvrj9nqw480dxcs";
        };

        # Branch coverage currently needs nightly rustc's unstable coverage
        # options, so keep that toolchain separate from the default build.
        nightlyToolchain = fenixPkgs.latest.withComponents [
          "cargo"
          "llvm-tools-preview"
          "rustc"
        ];

        # nixpkgs marks cargo-llvm-cov broken on Darwin because its own checks
        # fail there, but the binary works for this repository's macOS coverage.
        cargo-llvm-cov = pkgs.cargo-llvm-cov.overrideAttrs (old: {
          doCheck = false;
          meta = old.meta // {
            broken = false;
          };
        });

        craneLibBase = (crane.mkLib pkgs).overrideScope (_final: _prev: {
          inherit cargo-llvm-cov;
        });

        # Keep stable and nightly crane outputs separate so dependency caches
        # are not reused across incompatible Rust toolchains.
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
    ) // {
      darwinModules = {
        default = darwinModule;
        knoll = darwinModule;
      };

      homeModules = {
        default = homeModule;
        knoll = homeModule;
      };
    };
}
