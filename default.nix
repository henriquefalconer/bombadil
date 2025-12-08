{ rustPlatform, pkg-config, gitignoreSource, }:
rustPlatform.buildRustPackage rec {
  pname = "antithesis_browser";
  version = "0.1.0";

  src = gitignoreSource ./.;

  buildInputs = [ ];
  nativeBuildInputs = [ pkg-config ];
  cargoLock = { lockFile = ./Cargo.lock; };
}
