{
  description = "some-ting — TING handle → Quindar tones → Claude voice (push-to-talk)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        inherit (pkgs) lib stdenv;

        # System libraries the Rust crates link against:
        #   cpal       -> alsa-lib (Linux) / CoreAudio+AudioUnit (macOS)
        #   enigo      -> xdotool/libxdo (Linux) / CoreGraphics (macOS)
        #   tao+tray   -> gtk3 + libayatana-appindicator (Linux) / AppKit (macOS)
        #   x11rb      -> pure-Rust (no system lib)
        linuxDeps = with pkgs; [
          alsa-lib
          xdotool                 # provides libxdo (enigo X11 backend)
          gtk3
          libayatana-appindicator
          glib
          gdk-pixbuf
          cairo
          pango
          atk
        ];
        darwinDeps = with pkgs.darwin.apple_sdk.frameworks; [
          CoreAudio AudioUnit CoreFoundation CoreGraphics AppKit Foundation
        ];
        buildDeps =
          lib.optionals stdenv.isLinux linuxDeps
          ++ lib.optionals stdenv.isDarwin darwinDeps;

        rustPlatform = pkgs.rustPlatform;
        common = {
          version = "0.1.0";
          src = ./listener;
          cargoLock.lockFile = ./listener/Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config ]
            ++ lib.optionals stdenv.isLinux [ pkgs.wrapGAppsHook ];
          buildInputs = buildDeps;
        };
      in
      {
        packages = {
          # CLI (default features)
          listener = rustPlatform.buildRustPackage (common // {
            pname = "some-ting-listen";
            cargoBuildFlags = [ "--bin" "some-ting-listen" ];
          });
          # menu-bar GUI (gui feature)
          gui = rustPlatform.buildRustPackage (common // {
            pname = "some-ting";
            buildFeatures = [ "gui" ];
            cargoBuildFlags = [ "--bin" "some-ting" ];
          });
          default = self.packages.${system}.gui;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs;
            [ cargo rustc rustfmt clippy rust-analyzer pkg-config ]
            ++ lib.optionals stdenv.isLinux [ snixembed ];
          buildInputs = buildDeps;
          shellHook = lib.optionalString stdenv.isLinux ''
            export XDG_DATA_DIRS="${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
            echo "some-ting devshell: cargo + gtk3/alsa/libxdo + snixembed ready"
            echo "  cargo run --features gui --bin some-ting   # the tray"
            echo "  snixembed &                                 # bridge SNI -> i3bar"
          '';
        };
      });
}
