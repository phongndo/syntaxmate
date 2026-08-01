# Reproducible observatory deployment

This is a realistic Markdown runbook whose Nix examples are copied into a flake.
Operators preserve the labels café 東京 λ and the astral symbols 🚀 𝌆 verbatim.

## Flake outputs

The first block exercises arguments, recursive sets, inputs, interpolation, and
derivation-shaped values. It is documentation rather than a standalone Nix file.

```nix title="flake.nix" {data-example=primary}
{
  description = "Observatory console — café 東京 λ 🚀 𝌆";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
  };

  outputs = { self, nixpkgs, systems, ... }@inputs:
    let
      eachSystem = nixpkgs.lib.genAttrs (import systems);
      revision = self.shortRev or self.dirtyShortRev or "development";
    in {
      packages = eachSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          metadata = rec {
            pname = "observatory-console";
            version = "2.7.0";
            label = "${pname}-${version}";
          };
        in rec {
          default = console;
          console = pkgs.stdenv.mkDerivation {
            inherit (metadata) pname version;
            src = ./.;
            nativeBuildInputs = with pkgs; [ makeWrapper pkg-config ];
            buildInputs = [ pkgs.openssl pkgs.sqlite ];
            strictDeps = true;

            configurePhase = ''
              runHook preConfigure
              echo "revision=${revision}" > build-info
              echo "literal shell expansion: ''${HOME:-unknown}"
              runHook postConfigure
            '';

            buildPhase = ''
              runHook preBuild
              make MODE=release -j''${NIX_BUILD_CORES:-1}
              printf '%s\n' 'café 東京 λ 🚀 𝌆'
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              mkdir -p "$out/bin" "$out/share/observatory"
              cp console "$out/bin/observatory-console"
              cp build-info "$out/share/observatory/"
              wrapProgram "$out/bin/observatory-console" \
                --set OBSERVATORY_REVISION ${revision}
              runHook postInstall
            '';

            passthru = {
              inherit metadata;
              updateScript = ./nix/update.sh;
            };

            meta = with pkgs.lib; {
              description = "Field console for orbital weather observations";
              homepage = "https://example.test/observatory";
              license = licenses.mit;
              platforms = platforms.unix;
              mainProgram = "observatory-console";
            };
          };
        });
    };
}
```

The fence above is followed by ordinary Markdown, including **emphasis**, a
[runbook link](https://example.test/runbook), and an inline value `nix build`.

## NixOS module

The module below covers defaults, type expressions, assertions, conditionals,
attribute selection, dynamic names, paths, URLs, lists, and set operators.

~~~~NIX filename=module.nix
{ config, lib, pkgs, ... }:
with lib;
let
  cfg = config.services.observatory;
  endpointName = "OBSERVATORY_ENDPOINT";
  defaultPorts = [ 8080 8081 ];
  selectedPorts = if cfg.enableMetrics then defaultPorts else [ 8080 ];
  settings = {
    endpoint = cfg.endpoint;
    retries = cfg.retries;
    nested.mode = "safe";
    "quoted-key" = true;
    ${endpointName} = cfg.endpoint;
  };
  rendered = builtins.toJSON settings;
in {
  options.services.observatory = {
    enable = mkEnableOption "the observatory console";
    endpoint = mkOption {
      type = types.str;
      default = "https://api.example.test/v1";
      example = "http://127.0.0.1:9000";
      description = "Upstream telemetry endpoint.";
    };
    retries = mkOption {
      type = types.ints.between 0 12;
      default = 3;
    };
    enableMetrics = mkOption {
      type = types.bool;
      default = true;
    };
  };

  config = mkIf cfg.enable (mkMerge [
    {
      assertions = [{
        assertion = cfg.endpoint != "" && cfg.retries >= 0;
        message = "services.observatory.endpoint must not be empty";
      }];

      systemd.services.observatory = {
        description = "Observatory café collector";
        wantedBy = [ "multi-user.target" ];
        after = [ "network-online.target" ];
        environment = settings // {
          OBSERVATORY_PORTS = concatMapStringsSep "," toString selectedPorts;
        };
        serviceConfig = {
          ExecStart = "${pkgs.observatory}/bin/observatory-console";
          DynamicUser = true;
          StateDirectory = "observatory";
          Restart = "on-failure";
        };
      };

      environment.etc."observatory/settings.json".text = rendered;
    }
    (mkIf cfg.enableMetrics {
      networking.firewall.allowedTCPPorts = selectedPorts;
    })
  ]);
}
~~~~

## Small expression variants

Three-space indentation and a longer backtick delimiter exercise the fragment's
captured fence indentation and delimiter backreference.

   `````nix linenums=true
   let
     source = ../shared/source.nix;
     lookup = <nixpkgs>;
     values = [ 1 2 3 ] ++ [ 4 5 ];
     doubled = map (value: value * 2) values;
     keep = value: value > 2 && value <= 8;
     result = builtins.filter keep doubled;
     attrs = { alpha = 1; beta = 2; } // { beta = 20; gamma = 3; };
   in assert attrs ? gamma; {
     inherit source lookup result;
     fallback = attrs.missing or null;
     implication = (builtins.length result > 0) -> true;
   }
   `````

1. Build the flake.
2. Inspect the generated service environment.
3. Confirm that every fence is closed before publishing this runbook.
