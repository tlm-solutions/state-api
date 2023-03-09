{ naersk
, cmake
, lib
, openssl
, pkg-config
, postgresql_14
, protobuf
, src
, zlib
}:

naersk.buildPackage {
  pname = "state-api";
  version = "0.2.0";

  src = ./.;

  cargoSha256 = lib.fakeSha256;

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ protobuf postgresql_14 zlib openssl cmake ];

  meta = {
    description = "public api to fetch all the spicy live data";
    homepage = "https://github.com/tlm-solutions/state-api";
  };
}
