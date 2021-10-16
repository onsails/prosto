{
  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/21.05;
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = github:numtide/flake-utils;
  };

  outputs = { self, nixpkgs, utils, rust-overlay, ... }@inputs:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            rust-overlay.overlay
          ];
        };
      in
      {
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            # rust-bin.nightly.latest.default
            (rust-bin.fromRustupToolchainFile
              ./rust-toolchain.toml)
          ];
        };
      }
    );
}
