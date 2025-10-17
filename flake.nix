{
  description = "Keybinding hints plugin for zjstatus";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };

      rustWithWasiTarget = pkgs.rust-bin.stable.latest.default.override {
        targets = ["wasm32-wasip1"];
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain rustWithWasiTarget;

      zjstatus-hints = craneLib.buildPackage {
        src = craneLib.cleanCargoSource (craneLib.path ./.);
        cargoExtraArgs = "--target wasm32-wasip1";
        doCheck = false;
        doNotSign = true;
      };
    in {
      packages.default = zjstatus-hints;

      devShells.default = craneLib.devShell {
        packages = with pkgs; [
          rustWithWasiTarget
          wasmtime
        ];
      };
    });
}
