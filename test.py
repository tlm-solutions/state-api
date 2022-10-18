#!/usr/bin/env nix-shell
#!nix-shell -i python3.9 -p "python39.withPackages(ps: with ps; [ websockets requests ])"

import requests
import logging

# You must initialize logging, otherwise you'll not see debug output.
#logging.basicConfig()
#logging.getLogger().setLevel(logging.DEBUG)
#requests_log = logging.getLogger("requests.packages.urllib3")
#requests_log.setLevel(logging.DEBUG)
#requests_log.propagate = True

all_vehicles = requests.get('https://api.staging.dvb.solutions/vehicles/0/all').json()["network"]


for line, runs in all_vehicles.items():
    for run,tram in runs.items():
        r = requests.post('https://api.staging.dvb.solutions/vehicles/0/position', json={"line": int(line), "run": int(run) })

        if r.content:
            print(line, run)
            print(r.content)

