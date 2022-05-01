{ naersk, src, lib, pkg-config, cmake, protobuf }:

naersk.buildPackage {
  pname = "dvb-api";
  version = "0.1.0";

  src = ./.;

  cargoSha256 = lib.fakeSha256;

  nativeBuildInputs = [ pkg-config protobuf cmake ];

  meta = with lib; {
    description = "public api to fetch all the spicy live data";
    homepage = "https://github.com/dump-dvb/dvb-api";
  };
}
