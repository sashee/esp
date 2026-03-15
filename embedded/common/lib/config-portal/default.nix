let
  sources = {
    nixpkgs = builtins.fetchTarball {
      url = "https://github.com/NixOS/nixpkgs/archive/0590cd39f728e129122770c029970378a79d076a.tar.gz";
      sha256 = "1ia5kjykm9xmrpwbzhbaf4cpwi3yaxr7shl6amj8dajvgbyh2yh4";
    };
  };

  pkgs = import sources.nixpkgs { };
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
