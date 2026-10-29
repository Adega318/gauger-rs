{
  description = "A terminal resource monitor for Docker and Podman";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSystem = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      manifest = (lib.importTOML ./Cargo.toml).package;
    in
    {
      packages = forEachSystem (pkgs: rec {
        gauger = pkgs.rustPlatform.buildRustPackage {
          pname = manifest.name;
          inherit (manifest) version;

          # Only what cargo needs, so unrelated edits don't trigger rebuilds.
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
            ];
          };
          cargoLock.lockFile = ./Cargo.lock;

          meta = {
            inherit (manifest) description;
            homepage = "https://github.com/Adega318/gauger-rs";
            license = lib.licenses.mit;
            mainProgram = "gauger";
            platforms = lib.platforms.unix;
          };
        };
        default = gauger;
      });

      devShells = forEachSystem (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.default ];
          packages = with pkgs; [
            rustc
            clippy
            rustfmt
            rust-analyzer
          ];
          env = {
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            RUST_LOG = "info";
          };
        };
      });

      formatter = forEachSystem (pkgs: pkgs.nixfmt-rfc-style);
    };
}
