#!/usr/bin/env python3
# Written by Nesa. Claude Code runs this as its status line and sends the session as JSON on stdin.
# When Claude Code includes the account's rate limits, this keeps the 5-hour and 7-day windows in a
# small file for Nesa's usage bar. For each session it also keeps the context window and the input of the
# last call in a small file, for Nesa's context figure. It prints nothing, so the status line stays blank.
import json
import os
import re
import sys
import time

PATH = os.path.expanduser("~/.claude/nesa-usage.json")
CONTEXT_DIR = os.path.expanduser("~/.claude/nesa-context")

try:
    session = json.load(sys.stdin)
except Exception:
    sys.exit(0)


def write_json(path, value):
    # Write beside the file and rename, so Nesa never reads a half-written file.
    temp = f"{path}.{os.getpid()}.tmp"
    with open(temp, "w") as file:
        json.dump(value, file)
    os.replace(temp, path)


# The window, and the input of the last call. Claude Code's used_percentage counts the same input: input, cache
# writes and cache reads, without the output.
context = session.get("context_window") or {}
usage = context.get("current_usage") or {}
session_id = session.get("session_id")
if context.get("context_window_size") and usage and isinstance(session_id, str) and re.fullmatch(r"[0-9a-f-]{36}", session_id):
    input_tokens = sum(int(usage.get(key) or 0) for key in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))
    try:
        os.makedirs(CONTEXT_DIR, exist_ok=True)
        write_json(
            os.path.join(CONTEXT_DIR, f"{session_id}.json"),
            {
                "sessionId": session_id,
                "contextWindowSize": int(context["context_window_size"]),
                "inputTokens": input_tokens,
                "capturedAtMs": int(time.time() * 1000),
            },
        )
    except Exception:
        pass

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
    write_json(PATH, report)
