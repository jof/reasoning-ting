{
  description = "reasoning-ting — TING handle → Quindar tones → Claude voice (push-to-talk)";

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

        # Self-contained ALSA → PipeWire route for the bundled Linux binaries.
        # cpal speaks the ALSA *client* API; on a PipeWire host that's serviced
        # by the pipewire-alsa plugin. A hermetic binary links Nix's libasound,
        # whose plugin/config search paths point into the store — so we bundle
        # nixpkgs' OWN pipewire ALSA plugin (ABI-matched to this libasound) plus
        # the alsa-plugins set (jack/pulse, used by the `default` fallback chain)
        # and point libasound at them. The plugin connects to whatever PipeWire
        # daemon is running over its socket, so this works on NixOS *and* on
        # other distros (verified on Ubuntu). No host ALSA config needed.
        alsaPluginDir = pkgs.runCommand "reasoning-ting-alsa-plugins" { } ''
          mkdir -p $out/lib/alsa-lib
          ln -s ${pkgs.pipewire}/lib/alsa-lib/*     $out/lib/alsa-lib/
          ln -s ${pkgs.alsa-plugins}/lib/alsa-lib/* $out/lib/alsa-lib/
        '';
        alsaConf = pkgs.writeText "reasoning-ting-asound.conf" ''
          <${pkgs.alsa-lib}/share/alsa/alsa.conf>
          <${pkgs.pipewire}/share/alsa/alsa.conf.d/50-pipewire.conf>
          <${pkgs.pipewire}/share/alsa/alsa.conf.d/99-pipewire-default.conf>
        '';
        # Appended to gappsWrapperArgs (wrapGAppsHook3 wraps every bin/*).
        alsaGappsArgs = ''
          gappsWrapperArgs+=(
            --set ALSA_PLUGIN_DIR "${alsaPluginDir}/lib/alsa-lib"
            --set ALSA_CONFIG_PATH "${alsaConf}"
          )
        '';

        rustPlatform = pkgs.rustPlatform;
        common = {
          version = "0.1.0";
          src = ./listener;
          cargoLock.lockFile = ./listener/Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config ]
            ++ lib.optionals stdenv.isLinux [ pkgs.wrapGAppsHook3 ];
          buildInputs = buildDeps;
          # CLI + GUI both get the ALSA→PipeWire env; the GUI adds more below.
          preFixup = lib.optionalString stdenv.isLinux alsaGappsArgs;
        };
      in
      {
        # These packages are runnable Linux binaries: the wrapper bundles the
        # ALSA→PipeWire route (see alsaPluginDir/alsaConf above), so `nix run`/
        # `nix build` works on any host running PipeWire — no host ALSA config.
        # They're also the macOS .app and CI substrate. (A plain host-toolchain
        # `cargo build` + packaging/linux/install.sh is the lighter dev path.)
        packages = {
          # CLI (default features)
          listener = rustPlatform.buildRustPackage (common // {
            pname = "reasoning-ting-listen";
            cargoBuildFlags = [ "--bin" "reasoning-ting-listen" ];
          });
          # menu-bar GUI (gui feature)
          gui = rustPlatform.buildRustPackage (common // {
            pname = "reasoning-ting";
            buildFeatures = [ "gui" ];
            cargoBuildFlags = [ "--bin" "reasoning-ting" ];
            # Same ALSA→PipeWire env as the CLI, plus: libappindicator-sys
            # dlopen()s libayatana-appindicator3.so.1 by bare soname, so RPATH
            # doesn't find it — inject it on LD_LIBRARY_PATH via the gApps wrapper.
            preFixup = lib.optionalString stdenv.isLinux (alsaGappsArgs + ''
              gappsWrapperArgs+=(--prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath [ pkgs.libayatana-appindicator ]}")
            '');
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
            # So an in-shell `cargo run` finds the ALSA→PipeWire route too
            # (Nix's libasound otherwise can't see the host pipewire plugin).
            export ALSA_PLUGIN_DIR="${alsaPluginDir}/lib/alsa-lib"
            export ALSA_CONFIG_PATH="${alsaConf}"
            echo "reasoning-ting devshell: cargo + gtk3/alsa(+pipewire)/libxdo + snixembed + clippy"
            echo "  cargo run --features gui --bin reasoning-ting   # the tray (audio works in-shell)"
            echo "  cargo clippy --features gui  |  cargo test  |  snixembed &  (i3 tray host)"
          '';
        };
      });
}
