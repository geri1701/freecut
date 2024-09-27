{
  description = "A Rust development shell that's fltk-ready";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = [
        "x86_64-linux"
        "x86_64-darwin"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      perSystem = {
        config,
        pkgs,
        system,
        ...
      }: let
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [(import inputs.rust-overlay)];
        };
        makeRustInfo = {
          version,
          profile,
        }: let
          rust = pkgs.rust-bin.${version}.latest.${profile}.override {extensions = ["rust-src"];};
        in {
          name = "rust-" + version + "-" + profile;
          path = "${rust}/lib/rustlib/src/rust/library";
          drvs = [
            pkgs.rustc
            pkgs.cmake
            pkgs.git
            pkgs.gcc
            pkgs.rustfmt
            pkgs.pre-commit
            pkgs.rustPackages.clippy
            pkgs.xorg.libX11
            pkgs.xorg.libXext
            pkgs.xorg.libXinerama
            pkgs.xorg.libXcursor
            pkgs.xorg.libXrender
            pkgs.xorg.libXfixes
            pkgs.xorg.libXft
            pkgs.libcerf
            pkgs.libGL
            pkgs.mesa
            pkgs.pango
            pkgs.cairo
            pkgs.pkg-config
          ];
        };
        makeRustEnv = {
          version,
          profile,
        }: let
          rustInfo = makeRustInfo {
            inherit version profile;
          };
        in
          pkgs.buildEnv {
            name = rustInfo.name;
            paths = rustInfo.drvs;
          };

        matrix = {
          stable-default = {
            version = "stable";
            profile = "default";
          };
          stable-minimal = {
            version = "stable";
            profile = "minimal";
          };
          beta-default = {
            version = "beta";
            profile = "default";
          };
          beta-minimal = {
            version = "beta";
            profile = "minimal";
          };
          nightly-default = {
            version = "nightly";
            profile = "default";
          };
          nightly-minimal = {
            version = "nightly";
            profile = "minimal";
          };
        };
      in {
        formatter = pkgs.alejandra;
        devShells =
          builtins.mapAttrs
          (
            name: value: let
              version = value.version;
              profile = value.profile;
              rustInfo = makeRustInfo {
                inherit version profile;
              };
            in
              pkgs.mkShell {
                name = rustInfo.name;
                RUST_SRC_PATH = rustInfo.path;
                buildInputs = rustInfo.drvs;
              }
          )
          matrix
          // {
            default = let
              version = matrix.stable-default.version;
              profile = matrix.stable-default.profile;
              rustInfo = makeRustInfo {
                inherit version profile;
              };
            in
              pkgs.mkShell {
                name = rustInfo.name;
                RUST_SRC_PATH = rustInfo.path;
                LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath rustInfo.drvs;
                buildInputs = rustInfo.drvs;
                shellHook = ''
                  export CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG=true
                  export PATH=$PATH:''${CARGO_HOME:-~/.cargo}/bin
                  export PATH=$PATH:''${RUSTUP_HOME:-~/.rustup}/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/
                  echo ""
                  echo "Freecut development environment!" | ${pkgs.lolcat}/bin/lolcat
                  echo ""
                '';
              };
          };
      };
    };
}
