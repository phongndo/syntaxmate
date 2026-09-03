{
  description = "Syntaxmate development environment";

  inputs = {
    # Nixpkgs 26.05 is the last release with Intel macOS support.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    hk = {
      url = "github:jdx/hk/v1.54.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    hk,
    ...
  }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [rust-overlay.overlays.default];
          };
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          hkPackage = hk.packages.${system}.default.overrideAttrs {
            # hk 1.54.0 has a Git branch test that fails with Git 2.54.
            doCheck = false;
          };
        in {
          default = pkgs.mkShell {
            packages = [
              rustToolchain
              pkgs.nodejs_24
              pkgs.python3
              pkgs.git
              hkPackage
            ];

            CARGO_TERM_COLOR = "always";
            RUST_BACKTRACE = "1";
          };
        });
    };
}
