import asyncio
from websockets import connect

async def hello(uri):
    async with connect(uri) as websocket:
        await websocket.send("Hello world!")
        while True:
            print(await websocket.recv())

asyncio.run(hello("ws://socket.dvb.solutions"))
