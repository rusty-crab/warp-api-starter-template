#!/bin/bash

# Run server with cargo watch and systemfd for autoreload
systemfd --no-pid -s http::0.0.0.0:3535 -- cargo watch -x run

