{ pkgs, ... }:

pkgs.stdenv.mkDerivation {
  name = "flux-screensaver-windows-installer";
  src = ./.;

  nativeBuildInputs = [
    pkgs.mingwW64
    pkgs.SDL2-static
    pkgs.makeWrapper
  ];

  buildPhase = ''
    cargo build --target x86_64-pc-windows-gnu --release
  '';

  installPhase = ''
    mkdir -p $out/bin
    cp target/x86_64-pc-windows-gnu/release/flux.exe $out/bin/
  '';
}
