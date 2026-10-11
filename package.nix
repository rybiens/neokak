{
  lib,
  rustPlatform,
  makeWrapper,
  kakoune,
  git,
  ripgrep,
  perl,
  file,
}:

let
  cargoToml = lib.importTOML ./Cargo.toml;

  # Tools neokak runs by name (perl for Kakoune's `git update-diff`, which
  # draws the change markers; file for Kakoune's filetype detection of files
  # without an extension). They're appended to PATH, so the user's own
  # versions (e.g. a newer kak) still win.
  runtimeTools = [
    kakoune
    git
    ripgrep
    perl
    file
  ];
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./src
      ./rc
      ./tests
    ];
  };

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ makeWrapper ];

  # The e2e tests drive kak through a pseudo-terminal; fail instead of
  # skipping them if kak isn't found.
  nativeCheckInputs = runtimeTools;
  env.NEOKAK_REQUIRE_KAK = "1";

  checkFlags = [
    # Runs its commands with PATH=/usr/bin:/bin, which doesn't exist in the
    # build sandbox.
    "--skip=projenv::tests::runs_providers"
  ];

  postInstall = ''
    wrapProgram $out/bin/neokak --suffix PATH : ${lib.makeBinPath runtimeTools}
  '';

  meta = {
    inherit (cargoToml.package) description;
    homepage = "https://github.com/rybiens/neokak";
    license = with lib.licenses; [
      mit
      asl20
    ];
    mainProgram = "neokak";
    platforms = lib.platforms.linux;
  };
}
