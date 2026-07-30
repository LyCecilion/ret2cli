{
  description = "CLI client for Ret2Shell CTF platform";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        toolchain = fenix.packages.${system}.stable.withComponents [
          "cargo"
          "rustc"
          "rust-src"
          "rust-analyzer"
          "clippy"
          "rustfmt"
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            toolchain
            pkgs.pkg-config
            pkgs.nixfmt
          ];

          buildInputs = [
            pkgs.openssl
          ];

          shellHook = ''
            echo "ret2cli dev shell — $(rustc --version)"
          '';
        };

        formatter = pkgs.nixfmt;
      }
    );
}
