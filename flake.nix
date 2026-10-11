{
  description = "A Neovim-style IDE frontend for Kakoune";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        neokak = pkgs.callPackage ./package.nix { };
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.neokak;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.neokak ];
          packages = with pkgs; [
            clippy
            rustfmt
            rust-analyzer
          ];
        };
      });
    };
}
