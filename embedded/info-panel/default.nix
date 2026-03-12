let
  sources = {
    nixpkgs = builtins.fetchTarball {
      url = "https://github.com/NixOS/nixpkgs/archive/0590cd39f728e129122770c029970378a79d076a.tar.gz";
      sha256 = "1ia5kjykm9xmrpwbzhbaf4cpwi3yaxr7shl6amj8dajvgbyh2yh4";
    };

    rust-overlay = builtins.fetchTarball {
      url = "https://github.com/oxalica/rust-overlay/archive/2b18fe48d9a8a4ff3850d56b67cfe72f2a589237.tar.gz";
      sha256 = "055bx052hdawh0hp62v68mzsvgfrfz9i0y5s0l19capvvimkx8nj";
    };
  };

  pkgs = import sources.nixpkgs {
    overlays = [ (import sources.rust-overlay) ];
  };

  lib = pkgs.lib;
  storeDir = builtins.storeDir;
  repoRoot = ../..;
  appPath = ./.;
  appDir = lib.removePrefix "${toString repoRoot}/" (toString appPath);
  appManifest = builtins.fromTOML (builtins.readFile (appPath + "/Cargo.toml"));
  appName = appManifest.package.name;
  appVersion = appManifest.package.version;
  appBinName =
    if appManifest ? bin && appManifest.bin != [ ] then
      (builtins.head appManifest.bin).name
    else
      appName;
  artifactDirName = "${appName}-artifacts";
  firmwareRelPath = "firmware/${appBinName}";

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
              lib.removePrefix "${rootStr}/" pathStr;

          includePath = includedPath:
            rel == includedPath
            || lib.hasPrefix "${includedPath}/" rel
            || lib.hasPrefix "${rel}/" includedPath;
        in
        rel == "" || lib.any includePath includedPaths
      )
      [ ]
      repoRoot;

  filteredSrc = filterRepoSubtree [
    "embedded/common"
    appDir
  ];

  cargoDepsSrc = lib.cleanSourceWith {
    src = filteredSrc;
    filter = path: type:
      let
        pathStr = toString path;
        rootStr = toString filteredSrc;
        rel =
          if pathStr == rootStr then
            ""
          else
            lib.removePrefix "${rootStr}/" pathStr;
      in
      rel == ""
      || type == "directory"
      || lib.hasSuffix "/Cargo.toml" rel
      || lib.hasSuffix "/Cargo.lock" rel
      || lib.hasSuffix "/.cargo/config.toml" rel;
  };

  rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (
    toolchain:
    toolchain.default.override {
      extensions = [ "rust-src" ];
      targets = [ "riscv32imac-unknown-none-elf" ];
    }
  );

  stdManifest = "${rustToolchain}/lib/rustlib/src/rust/library/Cargo.toml";

  espIdfRev = "2c211b236707889e8400c4dc5644dd5c4ee071e0";

  espIdfFetched = pkgs.stdenvNoCC.mkDerivation {
    pname = "esp-idf-source";
    version = espIdfRev;
    nativeBuildInputs = [ pkgs.git pkgs.cacert ];

    outputHashMode = "recursive";
    outputHashAlgo = "sha256";
    outputHash = "sha256-ah7ErhyEbd9WfQaxOn/Ua2H/t06HFR6SVMn8D4gkyv0=";

    dontUnpack = true;
    dontConfigure = true;
    dontBuild = true;
    dontFixup = true;

    buildCommand = ''
      export HOME="$TMPDIR"
      export GIT_CONFIG_NOSYSTEM=1
      export GIT_TERMINAL_PROMPT=0
      export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"

      mkdir -p "$out/nix-support"

      git clone --no-checkout --filter=blob:none https://github.com/espressif/esp-idf.git "$out/source"
      git -C "$out/source" checkout ${espIdfRev}
      git -C "$out/source" submodule update --init --recursive --depth 1 --jobs $NIX_BUILD_CORES

      commit_timestamp=$(git -C "$out/source" show -s --format=%cI ${espIdfRev})

      printf '%s\n' '{"commitTimestamp":"'"$commit_timestamp"'","rev":"${espIdfRev}"}' > "$out/nix-support/esp-idf.json"

      find "$out/source" \( -type d -name .git -o -type f -name .git \) -print0 | xargs -0 rm -rf

      find "$out" -print0 | xargs -0 touch --date="$commit_timestamp"
    '';
  };

  espIdfSrc = "${espIdfFetched}/source";
  espIdfMeta = builtins.fromJSON (builtins.readFile "${espIdfFetched}/nix-support/esp-idf.json");

  espIdfVersionCmake = lib.replaceStrings ["\n"] [" "] (builtins.readFile "${espIdfSrc}/tools/cmake/version.cmake");
  espIdfVersionMatch = builtins.match ".*IDF_VERSION_MAJOR ([0-9]+).*IDF_VERSION_MINOR ([0-9]+).*IDF_VERSION_PATCH ([0-9]+).*" espIdfVersionCmake;
  espIdfVersionMajor = builtins.elemAt espIdfVersionMatch 0;
  espIdfVersionMinor = builtins.elemAt espIdfVersionMatch 1;
  espIdfVersionPatch = builtins.elemAt espIdfVersionMatch 2;
  espIdfVersion = "${espIdfVersionMajor}.${espIdfVersionMinor}.${espIdfVersionPatch}";
  espIdfConstraintVersion = "${espIdfVersionMajor}.${espIdfVersionMinor}";
  espIdfCommitTimestamp = espIdfMeta.commitTimestamp;

  espIdfConstraints = pkgs.fetchurl {
    url = "https://dl.espressif.com/dl/esp-idf/espidf.constraints.v${espIdfConstraintVersion}.txt";
    sha256 = "sha256-Ze7yyJ/glmhLsP2ltyRlFecCR4qRNH0PvimFyB6PDf0=";
  };

  # Switch back to pkgs.python3Packages.pip once the pinned nixpkgs package reaches a
  # version that supports `pip download --uploaded-prior-to` (monitor python3Packages.pip).
  pipBootstrapWheel = pkgs.fetchurl {
    url = "https://files.pythonhosted.org/packages/de/f0/c81e05b613866b76d2d1066490adf1a3dbc4ee9d9c839961c3fc8a6997af/pip-26.0.1-py3-none-any.whl";
    sha256 = "bdb1b08f4274833d62c1aa29e20907365a2ceb950410df15fc9521bad440122b";
  };

  espIdfTools = (builtins.fromJSON (builtins.readFile "${espIdfSrc}/tools/tools.json")).tools;

  toolsJsonPlatformMap = {
    x86_64-linux = "linux-amd64";
    aarch64-linux = "linux-arm64";
    armv6l-linux = "linux-armel";
    armv7l-linux = "linux-armhf";
    i686-linux = "linux-i686";
    x86_64-darwin = "macos";
    aarch64-darwin = "macos-arm64";
  };

  toolsJsonPlatform =
    toolsJsonPlatformMap.${pkgs.stdenv.buildPlatform.system}
    or (throw "Unsupported ESP-IDF tools.json platform: ${pkgs.stdenv.buildPlatform.system}");

  toolByName = name:
    let
      matches = builtins.filter (tool: tool.name == name) espIdfTools;
    in
    if matches == [ ] then
      throw "Tool ${name} not found in ESP-IDF tools.json"
    else
      builtins.head matches;

  recommendedVersion = tool:
    let
      version = lib.findFirst (candidate: (candidate.status or null) == "recommended") null tool.versions;
    in
    if version == null then
      throw "No recommended version found for ESP-IDF tool ${tool.name}"
    else
      version;

  espPythonPackages = pkgs.stdenvNoCC.mkDerivation {
    pname = "${appName}-esp-python-packages";
    version = espIdfVersion;
    dontUnpack = true;
    dontConfigure = true;
    dontBuild = true;
    dontFixup = true;

    nativeBuildInputs = [ pkgs.python3 pkgs.cacert ];

    outputHashMode = "recursive";
    outputHashAlgo = "sha256";
    outputHash = "sha256-FsO6QqKKO9F5Q+vk/Gm6mFWLgUdjG5kuv5qot+1wJu4=";

    installPhase = ''
      runHook preInstall

      export HOME="$TMPDIR"
      export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      export PIP_DISABLE_PIP_VERSION_CHECK=1
      export PYTHONPATH="${pipBootstrapWheel}"

      mkdir -p "$out"

      python -m pip download \
        --dest "$out" \
        --requirement "${espIdfSrc}/tools/requirements/requirements.core.txt" \
        --constraint "${espIdfConstraints}" \
        --uploaded-prior-to "${espIdfCommitTimestamp}"

      runHook postInstall
    '';
  };

  espPython = pkgs.stdenvNoCC.mkDerivation {
    pname = "${appName}-esp-python";
    version = espIdfVersion;
    dontUnpack = true;
    dontConfigure = true;
    dontBuild = true;

    nativeBuildInputs = [ pkgs.python3 pkgs.cacert ];

    installPhase = ''
      runHook preInstall

      export HOME="$TMPDIR"
      export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      export PIP_DISABLE_PIP_VERSION_CHECK=1
      export PYTHONPATH="${pipBootstrapWheel}"

      python -m venv "$out"

      python -m pip --python "$out/bin/python" install \
        --no-index \
        --find-links "${espPythonPackages}" \
        --constraint "${espIdfConstraints}" \
        --requirement "${espIdfSrc}/tools/requirements/requirements.core.txt"

      runHook postInstall
    '';
  };

  riscv32EspElfTool = toolByName "riscv32-esp-elf";
  riscv32EspElfVersion = recommendedVersion riscv32EspElfTool;
  riscv32EspElfArtifact =
    riscv32EspElfVersion.${toolsJsonPlatform}
    or (throw "ESP-IDF tool riscv32-esp-elf does not support platform ${toolsJsonPlatform}");

  riscv32EspElf = pkgs.stdenvNoCC.mkDerivation {
    pname = "riscv32-esp-elf";
    version = riscv32EspElfVersion.name;
    src = pkgs.fetchurl {
      url = riscv32EspElfArtifact.url;
      sha256 = riscv32EspElfArtifact.sha256;
    };
    dontUnpack = true;
    dontConfigure = true;
    dontBuild = true;
    nativeBuildInputs = [ pkgs.autoPatchelfHook pkgs.libarchive ];
    buildInputs = [ pkgs.stdenv.cc.cc.lib ];
    dontStrip = true;
    installPhase = ''
      runHook preInstall

      mkdir -p "$out"
      bsdtar -xf "$src" -C "$out" --strip-components 1

      runHook postInstall
    '';
  };

  cargoDeps = pkgs.stdenvNoCC.mkDerivation {
    pname = "${appName}-vendor";
    version = appVersion;
    src = cargoDepsSrc;
    dontConfigure = true;
    dontFixup = true;
    dontPatchShebangs = true;

    nativeBuildInputs = [ rustToolchain pkgs.cacert ];

    outputHashMode = "recursive";
    outputHashAlgo = "sha256";
    outputHash = "sha256-YjbiBRAp94oRdE9wJZT5t/L/ekgmWVzrPxwbLgT2ZYc=";

    installPhase = ''
      export HOME="$TMPDIR"
      export CARGO_HOME="$TMPDIR/cargo-home"
      export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      mkdir -p "$CARGO_HOME"

      cargo vendor \
        --locked \
        --versioned-dirs \
        --manifest-path "$src/${appDir}/Cargo.toml" \
        --sync "${stdManifest}" \
        "$out" \
        > /dev/null
    '';
  };

  nativeBuildInputs = with pkgs; [
    rustToolchain
    espflash
    ldproxy
    cmake
    ninja
    pkg-config
    git
    wget
    flex
    bison
    gperf
    espPython
    ccache
    llvmPackages.llvm
    llvmPackages.libclang
    dfu-util
  ];

  buildInputs = with pkgs; [
    openssl
    libffi
    libudev-zero
    libusb1
  ];
  firmware = pkgs.stdenv.mkDerivation {
    pname = "${appName}-firmware";
    version = appVersion;
    src = filteredSrc;
    dontConfigure = true;
    dontFixup = true;
    dontPatchELF = true;
    dontStrip = true;

    inherit nativeBuildInputs buildInputs;

    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";

    buildPhase = ''
      runHook preBuild

      export PATH="${lib.makeBinPath nativeBuildInputs}:$PATH"
      export HOME="$PWD"
      export CARGO_HOME="$PWD/.cargo-home"
      export CARGO_TARGET_DIR="$PWD/target"
      export APP_ARTIFACT_DIR="$NIX_BUILD_TOP/${artifactDirName}"
      export IDF_PATH="${espIdfSrc}"
      export ESP_IDF_TOOLS_INSTALL_DIR=fromenv
      export IDF_PYTHON_CHECK_CONSTRAINTS=no
      export IDF_PYTHON_ENV_PATH="${espPython}"
      export PATH="${riscv32EspElf}/bin:$PATH"
      export PATH="$IDF_PATH/tools:$PATH"
      mkdir -p "$CARGO_HOME"

      cd ${appDir}
      cp .cargo/config.toml .cargo/config.toml.orig
      cat >> .cargo/config.toml <<EOF

[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "${cargoDeps}"
EOF

      cargo build \
        -Z unstable-options \
        --artifact-dir "$APP_ARTIFACT_DIR" \
        --release \
        --locked \
        --offline

      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall

      mkdir -p "$out/firmware"
      install -m755 "$NIX_BUILD_TOP/${artifactDirName}/${appBinName}" "$out/${firmwareRelPath}"

      runHook postInstall
    '';
  };
in
pkgs.stdenvNoCC.mkDerivation {
  pname = appName;
  version = appVersion;
  dontUnpack = true;
  dontConfigure = true;
  dontBuild = true;

  passthru = {
    inherit
      pkgs
      lib
      repoRoot
      appPath
      appDir
      appName
      appVersion
      appBinName
    artifactDirName
      firmwareRelPath
      rustToolchain
      espIdfSrc
      espPython
      riscv32EspElf
      storeDir
      nativeBuildInputs
    buildInputs
      ;
    bashBin = "${pkgs.bashInteractive}/bin/bash";
    envBin = "${pkgs.coreutils}/bin/env";
    espflashPkg = pkgs.espflash;
    libclangPath = "${pkgs.llvmPackages.libclang.lib}/lib";
    sslCertFile = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
    idfToolsPath = "${espIdfSrc}/tools";
    wrappedEspflash = "${pkgs.espflash}/bin/espflash";
    bubblewrap = "${pkgs.bubblewrap}/bin/bwrap";
  };

  installPhase = ''
    runHook preInstall

    mkdir -p "$out/bin" "$out/firmware"

    ln -s "${firmware}/${firmwareRelPath}" "$out/${firmwareRelPath}"

    cat > "$out/bin/espflash" <<EOF
#!/bin/sh
set -eu

exec "${pkgs.bubblewrap}/bin/bwrap" \
  --unshare-all \
  --unshare-user \
  --die-with-parent \
  --new-session \
  --disable-userns \
  --ro-bind / / \
  --dev-bind /dev /dev \
  --proc /proc \
  --tmpfs /tmp \
  --tmpfs /run \
  "${pkgs.espflash}/bin/espflash" "\$@"
EOF

    cat > "$out/bin/flash" <<EOF
#!/bin/sh
set -eu

exec "$out/bin/espflash" flash "${firmware}/${firmwareRelPath}" "\$@"
EOF

    cat > "$out/bin/run" <<EOF
#!/bin/sh
set -eu

exec "$out/bin/espflash" flash --monitor "${firmware}/${firmwareRelPath}" "\$@"
EOF

    chmod 755 "$out/bin/espflash" "$out/bin/flash" "$out/bin/run"

    runHook postInstall
  '';
}
