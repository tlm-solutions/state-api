#!/usr/bin/env nix-shell
#!nix-shell -i python3.9 -p "python39.withPackages(ps: with ps; [ websockets requests ])"

import requests

all_vehicles = requests.get('https://api.staging.dvb.solutions/vehicles/0/all').json()["network"]


for line, runs in all_vehicles.items():
    for run,tram in runs.items():
        r = requests.post('https://api.staging.dvb.solutions/vehicles/0/position', json={"line": line, "run": run })

        print(r)

