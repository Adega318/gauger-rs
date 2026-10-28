{
  pkgs ? import <nixpkgs> { },
}:

pkgs.mkShell {
  packages = with pkgs; [
    cargo
    rustc
    rustfmt
    rust-analyzer
  ];
  env = {
    RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    RUST_LOG = "info";
  };

  shellHook = "";
}
