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

  repoRoot = ../..;
  libPath = ./.;
  libDir = pkgs.lib.removePrefix "${toString repoRoot}/" (toString libPath);

  filterRepoSubtree = includedPaths:
    pkgs.nix-gitignore.gitignoreFilterRecursiveSource
      (
        path: _type:
        let
          pathStr = toString path;
          rootStr = toString repoRoot;
          rel =
            if pathStr == rootStr then
              ""
            else
              pkgs.lib.removePrefix "${rootStr}/" pathStr;

          includePath = includedPath:
            rel == includedPath
            || pkgs.lib.hasPrefix "${includedPath}/" rel
            || pkgs.lib.hasPrefix "${rel}/" includedPath;
        in
        rel == "" || pkgs.lib.any includePath includedPaths
      )
      [ ]
      repoRoot;

  filteredSrc = filterRepoSubtree [
    "embedded/common"
    libDir
  ];

  src = pkgs.runCommand "info-panel-lib-src" { } ''
    mkdir -p $out/info-panel-lib $out/common/lib
    cp -r ${filteredSrc}/${libDir}/* $out/info-panel-lib/
    cp -r ${filteredSrc}/embedded/common/lib/* $out/common/lib/
  '';
in
rustPlatform.buildRustPackage {
  pname = manifest.package.name;
  version = manifest.package.version;
  inherit src;

  sourceRoot = "info-panel-lib-src/info-panel-lib";

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  cargoTestFlags = [ "--lib" "--test" "tests" ];

  passthru = {
    inherit pkgs rustPlatform;
  };
}
