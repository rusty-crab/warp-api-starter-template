#!/bin/bash

# postgres is just starting up, may not accept connections right away
sleep 5
# Run database migration fixes
movine init
movine status
movine fix

# Run server with cargo watch and systemfd for autoreload
systemfd --no-pid -s http::0.0.0.0:3535 -- cargo watch -x run

