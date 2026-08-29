#!/bin/bash
set -euo pipefail
cd /root/projects/gosh-appimage-manager-grok
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build build
ctest --test-dir build --output-on-failure
