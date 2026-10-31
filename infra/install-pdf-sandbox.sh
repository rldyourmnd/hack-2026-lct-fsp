#!/usr/bin/env bash
# Use one executable attachment: duplicate bwrap profiles prevent AppArmor transition.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
owned=/etc/apparmor.d/fsp-pdf-bwrap
# The distribution's own bwrap profile: /etc/apparmor.d/bwrap where present, and
# bwrap-userns-restrict on Ubuntu 26.04, which enables it by default. Either one
# already grants bwrap its user namespace; adding ours would duplicate the attachment.
packaged=""
for candidate in /etc/apparmor.d/bwrap /etc/apparmor.d/bwrap-userns-restrict; do
  if [[ -f "$candidate" ]]; then
    packaged="$candidate"
    break
  fi
done
if [[ -n "$packaged" ]]; then
  if [[ -f "$owned" ]]; then
    if grep -q '^fsp-pdf-bwrap ' /sys/kernel/security/apparmor/profiles; then
      apparmor_parser -R "$owned"
    fi
    rm "$owned"
  fi
  apparmor_parser -r "$packaged"
else
  install -D -m 644 "$here/apparmor/fsp-pdf-bwrap" "$owned"
  apparmor_parser -r "$owned"
fi
# Root is exempt from the Ubuntu user-namespace restriction; prove unprivileged execution.
runuser -u nobody -- /usr/bin/bwrap --unshare-all --die-with-parent \
  --ro-bind /usr /usr --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --dev /dev --tmpfs /tmp -- /usr/bin/true
