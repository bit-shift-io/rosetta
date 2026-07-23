#!/bin/bash
set -e
echo "docker.io login:"
podman login docker.io
podman build -t docker.io/gibbz/rosetta:latest .
read -p "press [enter] to upload to docker.io"
podman push docker.io/gibbz/rosetta:latest
