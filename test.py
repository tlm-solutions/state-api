#!/usr/bin/env nix-shell
#!nix-shell --pure -i python3.9 -p "python39Packes.ghcWithPackages (pkgs: [ pkgs.turtle ])"

import asyncio
from websockets import connect

async def hello(uri):
    async with connect(uri) as websocket:
        await websocket.send("Hello world!")
        while True:
            print(await websocket.recv())

asyncio.run(hello("ws://httsp://socket.dvb.solutions"))
