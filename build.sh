#!/bin/bash
set -e
echo "docker.io login:"
podman login docker.io
podman build -t docker.io/gibbz/rosetta:latest .
podman push docker.io/gibbz/rosetta:latest
