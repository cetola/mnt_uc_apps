#!/bin/bash

# Copyright (c) 2025 Stephano Cetola
# SPDX-License-Identifier: Apache-2.0

# Default intensity
intensity="${1:-255}"

# Validate: must be an integer between 1 and 255
if ! [[ "$intensity" =~ ^[0-9]+$ ]] || [ "$intensity" -lt 1 ] || [ "$intensity" -gt 255 ]; then
  echo "Error: Intensity must be a number between 1 and 255."
  exit 1
fi

while true; do
  status=$(cat /sys/class/power_supply/BAT0/status)
  charge=$(cat /sys/class/power_supply/BAT0/capacity)
  case "$status" in
    "Charging")
      if [ "$charge" -gt 99 ]; then
        echo "setled green $intensity" > /dev/ttyACM1
      else
        echo "setled blue $intensity" > /dev/ttyACM1
      fi
      ;;
    "Full")
      echo "setled green $intensity" > /dev/ttyACM1
      ;;
    "Discharging")
      echo "setled red $intensity" > /dev/ttyACM1
      ;;
  esac
  sleep .1
done

