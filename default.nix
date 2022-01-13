let
  rust_overlay = import (builtins.fetchTarball https://github.com/oxalica/rust-overlay/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> { overlays = [ rust_overlay ]; };
in
with nixpkgs;
pkgs.mkShell {
  buildInputs = with nixpkgs.latest.rustChannels.nightly; [
    alsa-lib
    cargo-bloat
    cargo-expand
    cargo-watch
    pkgconfig
    rust-bin.nightly.latest.default
    zlib
    zsh
  ];
}
  
  # See also
  # https://gitlab.com/sequoia-pgp/sequoia/-/commit/cbef891e687717b2c41f300f39a9ed9d42459e3c
  # https://github.com/oxalica/rust-overlay/issues/16
  # https://www.srid.ca/rust-nix
  # https://gutier.io/post/development-using-rust-with-nix/
