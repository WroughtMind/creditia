#!/usr/bin/env bash
# Native catalog rendering doesn't use WebKit or a desktop session. The build
# profile additionally compiles the isolated one-time legacy migration helper
# and operates installed windows. Both load Creditia's bundled Inter/Noto SC.
set -euo pipefail
profile=${1:?expected build, catalog or performance}
case "$profile" in build|catalog|performance) ;; *) exit 2 ;; esac
archive_dir=${RUNNER_TEMP:?}/gpui-apt
mkdir -p "$archive_dir"
sudo install -d -o _apt -g root -m 0700 "$archive_dir/partial"
apt_options=(-o Acquire::Retries=2 -o Acquire::http::Timeout=25 -o Acquire::https::Timeout=25 -o DPkg::Lock::Timeout=120 -o "Dir::Cache::archives=$archive_dir" -o APT::Keep-Downloaded-Packages=true -o Binary::apt::APT::Keep-Downloaded-Packages=true)
packages=(libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev libasound2-dev libvulkan1 vulkan-validationlayers mesa-vulkan-drivers)
for tool in gcc g++ clang cmake pkg-config; do
    if ! command -v "$tool" >/dev/null; then packages+=("$tool"); fi
done
if [[ "$profile" != catalog ]]; then
    packages+=(libwebkit2gtk-4.1-dev xvfb xauth dbus-x11 openbox xdotool xclip xdg-desktop-portal xdg-desktop-portal-gtk imagemagick x11-utils python3)
fi
if [[ "$profile" == performance ]]; then
    packages+=(fonts-noto-core fonts-noto-cjk)
fi
# Only Ubuntu supplies these packages. Auxiliary third-party package feeds
# on hosted runner images need not respond for a native desktop check to run.
if [[ -f /etc/apt/sources.list.d/ubuntu.sources ]]; then
    apt_options+=(-o Dir::Etc::sourcelist=/etc/apt/sources.list.d/ubuntu.sources -o Dir::Etc::sourceparts=-)
fi
# Try the validated APT cache first. A cache miss falls back to normal signed
# package lists and installation using the same distribution and dependencies.
if ! sudo env DEBIAN_FRONTEND=noninteractive apt-get "${apt_options[@]}" install --no-download --no-install-recommends -y "${packages[@]}"; then
    sudo apt-get "${apt_options[@]}" update
    sudo env DEBIAN_FRONTEND=noninteractive apt-get "${apt_options[@]}" install --no-install-recommends -y "${packages[@]}"
fi
# Cache only package files, never partial downloads or package-manager locks.
sudo rm -rf -- "$archive_dir/partial"
sudo rm -f -- "$archive_dir/lock"
mkdir -p "$RUNNER_TEMP/xdg-runtime"
chmod 700 "$RUNNER_TEMP/xdg-runtime"
printf 'XDG_RUNTIME_DIR=%s\n' "$RUNNER_TEMP/xdg-runtime" >> "$GITHUB_ENV"
