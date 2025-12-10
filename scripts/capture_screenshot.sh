#!/usr/bin/env bash
set -euo pipefail

OUT_PATH=${1:-assets/screenshots/v2_real.png}
RESOLUTION=${AIWINAMP_CAPTURE_RES:-1024x768x24}
mkdir -p "$(dirname "$OUT_PATH")"
ABS_OUT="$(python3 - <<'PY'
import os, sys
print(os.path.abspath(sys.argv[1]))
PY
"$OUT_PATH")"

xvfb-run -a -s "-screen 0 ${RESOLUTION}" bash -c "
set -euo pipefail
cargo +nightly run --release >/tmp/aiwinamp_capture.log 2>&1 &
APP_PID=\$!
sleep 5
import -display \"\$DISPLAY\" -window root \"$ABS_OUT\"
kill \$APP_PID
wait \$APP_PID || true
"

echo "Saved screenshot to $ABS_OUT"
