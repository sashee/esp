let
  nixpkgs = fetchTarball "https://github.com/NixOS/nixpkgs/tarball/nixos-24.11";
  pkgs = import nixpkgs { config = {}; overlays = []; };
in

pkgs.mkShellNoCC {
  packages = with pkgs; [
		glibcLocales
		rustc
		cargo
		cargo-binstall
    rustup
  ];
	LOCALE_ARCHIVE = "${pkgs.glibcLocales}/lib/locale/locale-archive";
	LC_ALL="en_US.UTF-8";
}

