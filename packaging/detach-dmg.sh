#!/usr/bin/env bash
# hdiutil uses 16 for EBUSY while macOS releases a recently inspected image.
# Retry detach after two and four seconds if the mounted image is still busy.
set -u
dmg_mount=${1:?Usage: detach-dmg.sh <mountpoint>}
for detach_delay in 0 2 4; do
  if (( detach_delay )); then
    echo "Disk image is still busy; retrying detach in ${detach_delay}s..." >&2
    sleep "$detach_delay"
  fi
  if hdiutil detach "$dmg_mount"; then
    exit 0
  else
    detach_status=$?
  fi
  if (( detach_status != 16 )); then exit "$detach_status"; fi
done
exit "$detach_status"
