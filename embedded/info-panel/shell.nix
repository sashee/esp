let
  package = import ./default.nix;
  shared = package.passthru;
  pkgs = shared.pkgs;

  build = pkgs.writeShellScriptBin "build" ''
    set -eu

    app_dir="''${ESP_INFO_PANEL_APP_DIR-}"
    artifact_dir="''${APP_ARTIFACT_DIR-}"

    if [ -z "$app_dir" ] || [ -z "$artifact_dir" ]; then
      printf '%s\n' 'build must be run from inside the info-panel nix-shell' >&2
      exit 1
    fi

    mkdir -p "$artifact_dir"

    cd "$app_dir"
    exec cargo build -Z unstable-options --artifact-dir "$artifact_dir" --release --locked "$@"
  '';

  flash = pkgs.writeShellScriptBin "flash" ''
    set -eu

    app_dir="''${ESP_INFO_PANEL_APP_DIR-}"
    artifact_dir="''${APP_ARTIFACT_DIR-}"

    if [ -z "$app_dir" ] || [ -z "$artifact_dir" ]; then
      printf '%s\n' 'flash must be run from inside the info-panel nix-shell' >&2
      exit 1
    fi

    artifact_path="$artifact_dir/${shared.appBinName}"

    build

    exec espflash flash "$artifact_path" "$@"
  '';

  run = pkgs.writeShellScriptBin "run" ''
    set -eu

    app_dir="''${ESP_INFO_PANEL_APP_DIR-}"
    artifact_dir="''${APP_ARTIFACT_DIR-}"

    if [ -z "$app_dir" ] || [ -z "$artifact_dir" ]; then
      printf '%s\n' 'run must be run from inside the info-panel nix-shell' >&2
      exit 1
    fi

    artifact_path="$artifact_dir/${shared.appBinName}"

    build

    exec espflash flash --monitor "$artifact_path" "$@"
  '';

  watch = pkgs.writeShellScriptBin "watch" ''
    set -eu

    app_dir="''${ESP_INFO_PANEL_APP_DIR-}"
    repo_root="''${ESP_INFO_PANEL_REPO_ROOT-}"

    if [ -z "$app_dir" ] || [ -z "$repo_root" ]; then
      printf '%s\n' 'watch must be run from inside the info-panel nix-shell' >&2
      exit 1
    fi

    cd "$repo_root"
    exec watchexec \
      --project-origin "$repo_root" \
      --watch "$repo_root/embedded" \
      --restart \
      --wrap-process=none \
      --shell=none \
      --stop-signal INT \
      --stop-timeout 1s \
      --debounce 100ms \
      --no-meta \
      -- run
  '';
in
pkgs.mkShell {
  name = "${shared.appName}-shell";

  nativeBuildInputs = shared.nativeBuildInputs ++ [
    pkgs.watchexec
    build
    flash
    run
    watch
  ];

  buildInputs = shared.buildInputs;

  IDF_PATH = shared.espIdfSrc;
  ESP_IDF_TOOLS_INSTALL_DIR = "fromenv";
  IDF_PYTHON_CHECK_CONSTRAINTS = "no";
  IDF_PYTHON_ENV_PATH = shared.espPython;
  LIBCLANG_PATH = shared.libclangPath;
  SSL_CERT_FILE = shared.sslCertFile;

  shellHook = ''
    export PATH="${shared.riscv32EspElf}/bin:${shared.idfToolsPath}:$PATH"

    repo_root=$(git rev-parse --show-toplevel 2>/dev/null) || {
      printf '%s\n' 'info-panel shell must be entered from inside a git worktree' >&2
      exit 1
    }

    export ESP_INFO_PANEL_REPO_ROOT="$repo_root"
    export ESP_INFO_PANEL_APP_DIR="$repo_root/${shared.appDir}"
    export CARGO_TARGET_DIR="$ESP_INFO_PANEL_APP_DIR/target"
    export APP_ARTIFACT_DIR="$ESP_INFO_PANEL_APP_DIR/.artifacts"

    mkdir -p "$CARGO_TARGET_DIR" "$APP_ARTIFACT_DIR"

    if [ -t 1 ]; then
      printf '%s\n' "info-panel nix-shell rooted at: $ESP_INFO_PANEL_REPO_ROOT"
      printf '%s\n' 'commands: build | flash | run | watch'
    fi
  '';
}
