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
      nixpkgs,
      flake-utils,
      fenix,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      workspace = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      version = workspace.package.version;
    in
    flake-utils.lib.eachSystem systems (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        ret2cli = pkgs.rustPlatform.buildRustPackage {
          pname = "ret2cli";
          inherit version;

          src = lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [ pkgs.pkg-config ];

          meta = {
            description = "CLI client for Ret2Shell CTF platform";
            homepage = "https://github.com/LyCecilion/ret2cli";
            license = lib.licenses.mit;
            mainProgram = "ret2cli";
          };
        };
        app = {
          type = "app";
          program = lib.getExe ret2cli;
          meta.description = "CLI client for Ret2Shell CTF platform";
        };
      in
      {
        packages = {
          inherit ret2cli;
          default = ret2cli;
        };

        apps = {
          ret2cli = app;
          default = app;
        };

        formatter = pkgs.nixfmt;
      }
      // lib.optionalAttrs (builtins.hasAttr system fenix.packages) {
        devShells.default =
          let
            toolchain = fenix.packages.${system}.stable.withComponents [
              "cargo"
              "rustc"
              "rust-src"
              "rust-analyzer"
              "clippy"
              "rustfmt"
            ];
          in
          pkgs.mkShell {
            nativeBuildInputs = [
              toolchain
              pkgs.cargo-dist
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
      }
    );
}
