{
  description = "Flake for backhand, a library and binaries for the reading, creating, and modification of SquashFS file systems";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };

        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib nixpkgs.legacyPackages.${system}).overrideToolchain rust;

        commonArgs = {
          pname = "backhand";
          version = "0.19.0";

          src = craneLib.cleanCargoSource self;
          strictDeps = true;
          nativeBuildInputs = with pkgs; [
            cmake
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in
      {
        packages = rec {
          backhand = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;

              doCheck = false;
            }
          );

          default = backhand;
        };

        devShells.default = craneLib.devShell {
          packages =
            with pkgs;
            [
              git
            ]
            ++ commonArgs.nativeBuildInputs;
        };
      }
    );
}
