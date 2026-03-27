let
  nixpkgs = import ../nixpkgs.nix;
  pkgs = import nixpkgs { };
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

  passthru = {
    inherit pkgs rustPlatform;
  };
}
