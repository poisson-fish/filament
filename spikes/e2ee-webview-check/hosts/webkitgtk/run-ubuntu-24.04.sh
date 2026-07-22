#!/bin/sh
set -eu

host_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
probe_directory=$(CDPATH= cd -- "$host_directory/../.." && pwd)
image_name=filament-webkitgtk-probe:ubuntu-24.04

docker build \
  --file "$host_directory/Dockerfile.ubuntu-24.04" \
  --tag "$image_name" \
  "$probe_directory" 1>&2

# WebKit's bubblewrap sandbox creates nested user/network namespaces. These
# capabilities are confined to this disposable, networkless container; the
# WebKit sandbox itself remains enabled.
exec docker run --rm \
  --network none \
  --memory 2g \
  --pids-limit 512 \
  --shm-size 256m \
  --cap-add SYS_ADMIN \
  --cap-add NET_ADMIN \
  --security-opt seccomp=unconfined \
  --env GTK_A11Y=none \
  "$image_name"
