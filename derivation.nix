{ naersk, src, lib, pkg-config, cmake, protobuf, postgresql, zlib }:

naersk.buildPackage {
  pname = "state-api";
  version = "0.1.3";

  src = ./.;

  cargoSha256 = lib.fakeSha256;

  nativeBuildInputs = [ pkg-config protobuf cmake postgresql zlib ];

  meta = with lib; {
    description = "public api to fetch all the spicy live data";
    homepage = "https://github.com/tlm-solutions/state-api";
  };
}
