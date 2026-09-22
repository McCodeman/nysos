# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

{
  description = "nysos terminal demo application and development toolchain";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  # Nixpkgs 26.11 dropped Intel macOS; retain its maintained 26.05 toolchain.
  inputs.nixpkgs-intel-mac.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";

  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-intel-mac,
    }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      pkgsFor =
        system:
        import (if system == "x86_64-darwin" then nixpkgs-intel-mac else nixpkgs) { inherit system; };
      packageFor =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = "nysos";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./build.rs
              ./src
              ./examples
              ./docs/man
              ./docs/reference/cli.md
              ./Makefile
              ./LICENSE
              ./NOTICE
            ];
          };
          # Flake sources exclude .git; preserve the revision supplied by Nix.
          env.NYSOS_GIT_COMMIT = self.rev or self.dirtyRev or "unknown";
          env.NYSOS_GIT_TAG = "unknown";
          env.NYSOS_GIT_DESCRIBE = self.shortRev or self.dirtyShortRev or "unknown";
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.installShellFiles ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
            pkgs.libiconv
            pkgs.apple-sdk
          ];
          # Tests spawn real shells; /bin/sh is absent in the Linux build sandbox.
          env.NYSOS_TEST_SHELL = "${pkgs.bash}/bin/bash";
          env.NYSOS_TEST_BASH = "${pkgs.bash}/bin/bash";
          env.NYSOS_TEST_ZSH = "${pkgs.zsh}/bin/zsh";
          postInstall = ''
            installManPage docs/man/nysos.1
            install -Dm644 LICENSE $out/share/doc/nysos/LICENSE
            install -Dm644 NOTICE $out/share/doc/nysos/NOTICE
          '';
          meta = {
            description = "Multi-pane scripted and interactive terminal demonstrations";
            license = pkgs.lib.licenses.asl20;
            mainProgram = "nysos";
            platforms = systems;
          };
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          package = packageFor (pkgsFor system);
        in
        {
          default = package;
          nysos = package;
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/nysos";
          meta.description = "Run nysos";
        };
      });

      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];
            packages =
              with pkgs;
              [
                cargo
                cargo-sbom
                rustc
                rustfmt
                clippy
                rust-analyzer
                gnumake
                git
                gitsign
                tmux
                pkg-config
                python313
                uv
                mandoc
                zsh
                nixfmt
              ]
              ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.xdg-utils ];
            UV_PYTHON = "${pkgs.python313}/bin/python3";
            UV_PYTHON_DOWNLOADS = "never";
            NYSOS_TEST_SHELL = "${pkgs.bash}/bin/bash";
            NYSOS_TEST_BASH = "${pkgs.bash}/bin/bash";
            NYSOS_TEST_ZSH = "${pkgs.zsh}/bin/zsh";
          };
        }
      );

      checks = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          # Reuses the package build and its real-PTY tests.
          package = self.packages.${system}.default;
          quality = self.packages.${system}.default.overrideAttrs (old: {
            pname = "nysos-quality";
            nativeBuildInputs = old.nativeBuildInputs ++ [
              pkgs.rustfmt
              pkgs.clippy
              pkgs.gnumake
              pkgs.mandoc
            ];
            preCheck = ''
              cargo fmt --all -- --check
              cargo clippy --offline --locked --all-targets -- -D warnings
              cargo run --offline --locked --example generate_docs -- --check
              cargo run --offline --locked -- --config examples/demo.toml --check
              mandoc -T lint docs/man/nysos.1
            '';
          });
        }
      );

      formatter = forAllSystems (system: (pkgsFor system).nixfmt);
    };
}
