{
  description = "ooze";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
          config = {
            allowUnfreePredicate = pkg: builtins.elem (nixpkgs.lib.getName pkg) [
              "claude-code"
            ];
          };
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "llvm-tools-preview" ];
        };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Crane's default filter keeps only Cargo sources. The crate also needs
        # the tree-sitter query files it embeds via include_str! (*.scm) and the
        # test fixtures the test suite reads at runtime (tests/fixtures/**).
        src =
          let
            keepExtra = path:
              nixpkgs.lib.hasSuffix ".scm" path
              || builtins.match ".*/tests/fixtures/.*" path != null;
          in
          nixpkgs.lib.cleanSourceWith {
            src = self;
            name = "ooze-source";
            filter = path: type:
              keepExtra path || craneLib.filterCargoSources path type;
          };

        commonArgs = {
          inherit src;
          strictDeps = true;

          # tree-sitter grammar crates compile bundled C sources via cc.
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ];
        };

        # Build all dependencies separately so they cache across source changes.
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        ooze = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;

          meta = {
            description = "ooze";
            mainProgram = "ooze";
          };
        });
      in
      {
        packages.default = ooze;
        packages.ooze = ooze;

        apps.default = flake-utils.lib.mkApp { drv = ooze; };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config
            openssl
            jq
            sccache
            claude-code
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LLVM_COV = "${rustToolchain}/lib/rustlib/${pkgs.stdenv.hostPlatform.rust.rustcTarget}/bin/llvm-cov";
          LLVM_PROFDATA = "${rustToolchain}/lib/rustlib/${pkgs.stdenv.hostPlatform.rust.rustcTarget}/bin/llvm-profdata";

          shellHook = ''
            alias ooze="./result/bin/ooze"
          '';
        };
      }
    );
}
