#!/bin/sh
# Compatibility entry point. Phase 8 replaces incomplete app staging with audited local packaging.
set -eu
exec bash "$(dirname "$0")/release/macos.sh" unsigned
