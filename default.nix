let
  pkgs = import <nixpkgs> {};
in
  pkgs.mkShell {
    buildInputs = with pkgs; [
        alsa-lib
        cargo
        clippy
        pkg-config
    ];
  }
  