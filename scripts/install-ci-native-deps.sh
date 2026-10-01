#!/usr/bin/env bash
# The Azure mirror can stall after serving metadata. Keep the signed Ubuntu
# archive and bounded network waits; never accept unsigned packages or stale indexes.
set -euo pipefail

if [[ -f /etc/apt/apt-mirrors.txt ]]; then
  printf '%s\n' 'https://archive.ubuntu.com/ubuntu/' |
    sudo tee /etc/apt/apt-mirrors.txt >/dev/null
fi

apt_options=(-o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 -o Acquire::Retries=2)
sudo apt-get "${apt_options[@]}" -o APT::Update::Error-Mode=any update
sudo apt-get "${apt_options[@]}" install -y "$@"
