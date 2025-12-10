#!/usr/bin/env bash
set -euo pipefail

OUT_PATH=${1:-assets/screenshots/v2_real.png}
RESOLUTION=${AIWINAMP_CAPTURE_RES:-1024x768x24}
WINDOW_TITLE=${AIWINAMP_CAPTURE_TITLE:-AIWinAmp}
mkdir -p "$(dirname "$OUT_PATH")"
ABS_OUT="$(python3 -c 'import os,sys; print(os.path.abspath(sys.argv[1]))' "$OUT_PATH")"

xvfb-run -a -s "-screen 0 ${RESOLUTION}" bash -c "
set -euo pipefail
cargo +nightly run --release >/tmp/aiwinamp_capture.log 2>&1 &
APP_PID=\$!
for _ in {1..20}; do
    WINDOW_ID=\$(xdotool search --name \"${WINDOW_TITLE}\" || true)
    if [ -n \"\$WINDOW_ID\" ]; then
        break
    fi
    sleep 0.5
done
if [ -z \"\$WINDOW_ID\" ]; then
    echo \"Failed to find window titled ${WINDOW_TITLE}\" >&2
    kill \$APP_PID
    wait \$APP_PID || true
    exit 1
fi
sleep 0.5
import -display \"\$DISPLAY\" -window \"\$WINDOW_ID\" \"$ABS_OUT\"
kill \$APP_PID
wait \$APP_PID || true
"

echo "Saved screenshot to $ABS_OUT"
