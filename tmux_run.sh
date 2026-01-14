#!/usr/bin/env bash
set -euo pipefail
cd "$HOME/ag"
: > qodo_gui_launch.log
: > qodo_pane.log
: > server.log
{
  echo "==== q --login output ===="
  QODO_LOG_LEVEL=debug q --login
} &> qodo_gui_launch.log
{
  echo "==== q --gui output ===="
  QODO_LOG_LEVEL=debug q --gui
} &> qodo_pane.log
