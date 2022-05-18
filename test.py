#!/usr/bin/env nix-shell
#!nix-shell --pure -i python3.9 -p "python39Packes.ghcWithPackages (pkgs: [ pkgs.turtle ])"

import asyncio
import json
from websockets import connect

config = {
    "regions": [0, 1],
}

raw_config = json.dumps(config);

async def hello(uri):
    async with connect(uri) as websocket:
        await websocket.send(raw_config)
        while True:
            print(await websocket.recv())

asyncio.run(hello("wss://socket.staging.dvb.solutions"))
