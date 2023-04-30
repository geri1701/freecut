{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = naersk-lib.buildPackage ./.;
<<<<<<< HEAD
        devShell = with pkgs; mkShell {
          buildInputs = [ cargo rustc rustfmt pre-commit rustPackages.clippy ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
        };
=======
        devShell = with pkgs;
          mkShell {
            buildInputs = [
              cargo
              rustc
              rustfmt
              pre-commit
              rustPackages.clippy
              xorg.libX11
              xorg.libXext
              xorg.libXinerama
              xorg.libXcursor
              xorg.libXrender
              xorg.libXfixes
              xorg.libXft
              pango
              curl
              openssl
              openssl.dev
              pkg-config
            ];
            nativeBuildInputs = with pkgs; [ pkg-config openssl.dev curl ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
          };
>>>>>>> 177e1a7 (Remove workflow)
      });
}
