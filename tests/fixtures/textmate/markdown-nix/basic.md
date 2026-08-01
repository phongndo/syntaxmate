# Nix deployment example

This Markdown host keeps the café 東京 λ release reproducible for 🚀 and 𝌆.

```nix title="flake-module.nix"
{ lib, pkgs, ... }:
let
  greeting = "hello from ${pkgs.system}";
  ports = [ 80 443 ];
in {
  services.example = {
    enable = true;
    message = greeting;
    inherit ports;
  };
  environment.systemPackages = with pkgs; [ curl git ];
}
```

The surrounding prose confirms that the closing fence returns to Markdown.
