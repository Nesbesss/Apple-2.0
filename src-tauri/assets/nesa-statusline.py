#!/usr/bin/env python3
# Written by Nesa. Claude Code runs this as its status line and sends the session as JSON on stdin.
# When Claude Code includes the account's rate limits, this keeps the 5-hour and 7-day windows in a
# small file for Nesa's usage bar. It prints nothing, so the status line stays blank.
import json
import os
import sys
import time

PATH = os.path.expanduser("~/.claude/nesa-usage.json")

try:
    session = json.load(sys.stdin)
except Exception:
    sys.exit(0)

limits = session.get("rate_limits") or {}
windows = {}
for source, name in (("five_hour", "fiveHour"), ("seven_day", "sevenDay")):
    window = limits.get(source) or {}
    if "used_percentage" in window and "resets_at" in window:
        windows[name] = {
            "usedPercent": float(window["used_percentage"]),
            "resetsAtMs": int(window["resets_at"]) * 1000,
        }

if windows:
    # Claude Code drops a window once its reset has passed. Keep the last report of that window, so Nesa
    # can show it as unused after the reset instead of losing it.
    try:
        with open(PATH) as file:
            report = json.load(file)
    except Exception:
        report = {}
    report.update(windows)
    report["capturedAtMs"] = int(time.time() * 1000)
    # Write beside the file and rename, so Nesa never reads a half-written report.
    temp = f"{PATH}.{os.getpid()}.tmp"
    with open(temp, "w") as file:
        json.dump(report, file)
    os.replace(temp, PATH)
