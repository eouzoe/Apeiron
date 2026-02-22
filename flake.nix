{
  description = "Forge — deterministic execution fabric (Nix + Firecracker + SHA-256)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, fenix, crane, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        toolchain = fenix.packages.${system}.combine [
          fenix.packages.${system}.stable.cargo
          fenix.packages.${system}.stable.rustc
          fenix.packages.${system}.stable.rust-src
          fenix.packages.${system}.stable.clippy
          fenix.packages.${system}.stable.rustfmt
          fenix.packages.${system}.stable.rust-analyzer
          fenix.packages.${system}.targets.wasm32-unknown-unknown.stable.rust-std
          fenix.packages.${system}.targets.x86_64-unknown-linux-musl.stable.rust-std
        ];

        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        forge = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in
      {
        packages.default = forge;

        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            # Cargo tools
            cargo-nextest
            cargo-llvm-cov
            cargo-deny
            cargo-machete
            cargo-flamegraph
            cargo-watch
            cargo-expand

            # Fuzzing
            cargo-fuzz

            # WASM
            wasm-pack
            wasm-bindgen-cli

            # Nix tools
            nil
            nixpkgs-fmt

            # Dev tools
            bun
            jq
            taplo
            direnv
            git
            gh

            # forge: Firecracker microVM runtime
            firecracker
          ];

          shellHook = ''
            echo "Rust $(rustc --version)"
            echo "Forge devshell loaded"
          '';
        };
      }
    );
}
