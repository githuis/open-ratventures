#!/usr/bin/env bash
RATQUEST_HOST=https://play.ratventure.online/api/ trunk build --release --config Trunk.production.toml
read -p "Press Enter to close..."
