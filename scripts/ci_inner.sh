#!/usr/bin/env bash
# Runs the harness for one framework. On Linux this must run inside Xvfb (see ci.yml).
set -u
fw=$1
RUNS=${RUNS:-2}
if [ "$(uname -s)" = Linux ]; then
  # GPUI's X11 frame loop only starts once a window manager has mapped the window
  openbox >/dev/null 2>&1 &
  sleep 2
fi
rc=0
run() { python scripts/xbench.py --fw "$fw" --scenarios commits,grid --runs "$RUNS" --out results --data data "$@" || rc=1; }
if [ "$fw" = tauri ] && [ "$(uname -s)" = Linux ]; then
  # WebKitGTK: compare the default renderer against the DMABUF renderer disabled
  run --tag webkit-default
  run --tag dmabuf-off --env WEBKIT_DISABLE_DMABUF_RENDERER=1
else
  run --tag default
fi
exit $rc
