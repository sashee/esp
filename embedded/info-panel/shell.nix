let
  package = import ./default.nix;
  shared = package.passthru;
  pkgs = shared.pkgs;

  sandboxEntrypoint = pkgs.writeShellScript "info-panel-shell-entrypoint" ''
    set -eu

    mkdir -p "$HOME"

    exec "$@"
  '';

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
    export CARGO_HOME="$ESP_INFO_PANEL_REPO_ROOT/.cargo-home"
    export CARGO_TARGET_DIR="$ESP_INFO_PANEL_APP_DIR/target"
    export APP_ARTIFACT_DIR="$ESP_INFO_PANEL_APP_DIR/.artifacts"

    mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR" "$APP_ARTIFACT_DIR"

    if [ "''${ESP_BWRAP_SHELL-}" = 1 ]; then
      trap - EXIT
      exitHooks=()
      failureHooks=()
      _nix_shell_clean_tmpdir() { :; }

      if [ -t 1 ]; then
        printf '%s\n' "info-panel nix-shell sandboxed to: $ESP_INFO_PANEL_REPO_ROOT"
        printf '%s\n' 'commands: build | flash | run | watch'
      fi
      return
    fi

    shell_arg0=$(awk 'BEGIN { RS = "\\0" } NR == 1 { print; exit }' "/proc/$$/cmdline")
    shell_arg1=$(awk 'BEGIN { RS = "\\0" } NR == 2 { print; exit }' "/proc/$$/cmdline")
    shell_arg2=$(awk 'BEGIN { RS = "\\0" } NR == 3 { print; exit }' "/proc/$$/cmdline")

    shell_argv=("$shell_arg0")
    if [ "$shell_arg1" = "--rcfile" ]; then
      shell_argv+=("$shell_arg1" "$shell_arg2")
    elif [ -n "$shell_arg1" ]; then
      shell_argv+=("$shell_arg1")
    fi

    bwrap_args=(
      --unshare-all
      --share-net
      --unshare-user
      --die-with-parent
      --disable-userns
      --ro-bind "${shared.storeDir}" "${shared.storeDir}"
      --bind "$ESP_INFO_PANEL_REPO_ROOT" "$ESP_INFO_PANEL_REPO_ROOT"
      --dev-bind /dev /dev
      --proc /proc
      --ro-bind /sys /sys
      --tmpfs /tmp
      --tmpfs /run
      --dir /bin
      --dir /usr
      --dir /usr/bin
      --dir /tmp/home
      --ro-bind /etc /etc
      --symlink "${shared.bashBin}" /bin/bash
      --symlink "${shared.bashBin}" /bin/sh
      --symlink "${shared.envBin}" /usr/bin/env
      --setenv HOME /tmp/home
      --setenv SHELL "${shared.bashBin}"
      --setenv BASH "${shared.bashBin}"
      --setenv ESP_BWRAP_SHELL 1
      --setenv ESP_INFO_PANEL_REPO_ROOT "$ESP_INFO_PANEL_REPO_ROOT"
      --setenv ESP_INFO_PANEL_APP_DIR "$ESP_INFO_PANEL_APP_DIR"
      --setenv CARGO_HOME "$CARGO_HOME"
      --setenv CARGO_TARGET_DIR "$CARGO_TARGET_DIR"
      --setenv APP_ARTIFACT_DIR "$APP_ARTIFACT_DIR"
      --setenv PATH "$PATH"
      --setenv IDF_PATH "$IDF_PATH"
      --setenv ESP_IDF_TOOLS_INSTALL_DIR "$ESP_IDF_TOOLS_INSTALL_DIR"
      --setenv IDF_PYTHON_CHECK_CONSTRAINTS "$IDF_PYTHON_CHECK_CONSTRAINTS"
      --setenv IDF_PYTHON_ENV_PATH "$IDF_PYTHON_ENV_PATH"
      --setenv LIBCLANG_PATH "$LIBCLANG_PATH"
      --setenv SSL_CERT_FILE "$SSL_CERT_FILE"
      --setenv IN_NIX_SHELL "$IN_NIX_SHELL"
      --setenv NIX_BUILD_TOP "''${NIX_BUILD_TOP-}"
      --chdir "$PWD"
    )

    resolv_target=$(readlink -f /etc/resolv.conf 2>/dev/null || true)
    if [ -n "$resolv_target" ] && [ -e "$resolv_target" ] && [ "$resolv_target" != /etc/resolv.conf ]; then
      bwrap_args+=(--ro-bind "$resolv_target" "$resolv_target")
    fi

    if [ "$shell_arg1" = "--rcfile" ] && [ -n "$shell_arg2" ] && [ -e "$shell_arg2" ]; then
      rc_dir=$(dirname "$shell_arg2")
      bwrap_args+=(--bind "$rc_dir" "$rc_dir")
    elif [ -n "$shell_arg1" ] && [ -e "$shell_arg1" ]; then
      rc_dir=$(dirname "$shell_arg1")
      bwrap_args+=(--bind "$rc_dir" "$rc_dir")
    fi

    unset NIX_ENFORCE_PURITY

    exec "${shared.bubblewrap}" "''${bwrap_args[@]}" -- "${sandboxEntrypoint}" "''${shell_argv[@]}"
  '';
}
