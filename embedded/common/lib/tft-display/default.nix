let
  nixpkgs = import ../../../nixpkgs.nix;
  pkgs = import nixpkgs { };
  rustPlatform = pkgs.rustPlatform;
  manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = manifest.package.name;
  version = manifest.package.version;
  src = pkgs.lib.cleanSource ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  cargoTestFlags = [ "--lib" ];

  passthru = {
    inherit pkgs rustPlatform;
  };
}
