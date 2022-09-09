# DVB API

![](https://img.shields.io/endpoint?url=https%3A%2F%2Fhydra.hq.c3d2.de%2Fjob%2Fdvb-dump%2Fdvb-api%2Fdvb-api.x86_64-linux%2Fshield)
![built with nix](https://builtwithnix.org/badge.svg)

**Contact:** <dump@dvb.solutions>

This service models the public transport network of multiple cities. Against this model you can REST request and overview of endpoints are documented [here](https://docs.staging.dvb.solutions/chapter_5_1_api.html).

## Configuration

### Environment Variables

- **GRAPH_FILE** default: graph.json
- **STOPS_FILE** default: ../stops.json
- **GRPC_HOST** default: 127.0.0.1:50051
- **HTTP_HOST** default: 127.0.0.1
- **HTTP_PORT** default: 9002

