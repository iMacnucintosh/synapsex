#!/usr/bin/env python3
"""Capture Basilisk V4 Pro button events for Synapsex identify-mapping.

Usage:
  python3 scripts/probe-buttons.py

Then press, one by one, the buttons that do NOT highlight in the UI
(Profile M1, DPI, HyperShift, Side AI, …). Ctrl+C to stop.

Notes:
  - Classic clicks / side / wheel appear on /dev/input/event*.
  - Factory DPI emits a hidraw interrupt: report 05 56 <stage>.
  - Factory Profile / HyperShift / disabled Side AI usually emit nothing on evdev;
    Profile is visible only as an active-slot change (feature report poll).
"""

from __future__ import annotations

import glob
import os
import select
import struct
import sys
import time

EV_KEY = 1
EV_REL = 2
EV_MSC = 4

BTN = {
    0x110: "BTN_LEFT",
    0x111: "BTN_RIGHT",
    0x112: "BTN_MIDDLE",
    0x113: "BTN_SIDE",
    0x114: "BTN_EXTRA",
    0x115: "BTN_FORWARD",
    0x116: "BTN_BACK",
    0x117: "BTN_TASK",
}
for i in range(0x118, 0x130):
    BTN[i] = f"BTN_{i:#x}"

KEY_NAMED = {
    1: "ESC",
    28: "ENTER",
    57: "SPACE",
    **{59 + i: f"F{i+1}" for i in range(10)},
    87: "F11",
    88: "F12",
    **{183 + i: f"F{13+i}" for i in range(12)},
}

REL = {6: "HWHEEL", 8: "WHEEL", 11: "WHEEL_HI", 12: "HWHEEL_HI"}


def basilisk_nodes() -> list[tuple[str, str]]:
    text = open("/proc/bus/input/devices").read()
    out: list[tuple[str, str]] = []
    vendor_ok = product_ok = False
    name = ""
    for line in text.splitlines():
        if line.startswith("I"):
            low = line.lower()
            vendor_ok = "vendor=1532" in low
            product_ok = "product=00e3" in low or "product=00e2" in low
        elif line.startswith("N:"):
            name = line.split("=", 1)[-1].strip().strip('"')
        elif line.startswith("H") and vendor_ok and product_ok:
            for part in line.replace("Handlers=", " ").split():
                if part.startswith("event") and part[5:].isdigit():
                    out.append((f"/dev/input/{part}", name))
        elif not line.strip():
            vendor_ok = product_ok = False
            name = ""
    return out


def basilisk_hidraw() -> list[str]:
    out: list[str] = []
    for uevent in sorted(glob.glob("/sys/class/hidraw/hidraw*/device/uevent")):
        text = open(uevent).read().lower()
        if "00001532:" not in text:
            continue
        if ":000000e3" not in text and ":000000e2" not in text:
            continue
        name = uevent.split("/")[4]
        out.append(f"/dev/{name}")
    return out


def main() -> None:
    nodes = basilisk_nodes()
    hidraws = basilisk_hidraw()
    if not nodes and not hidraws:
        print("No Basilisk nodes found (VID 1532, PID 00e2/00e3).", file=sys.stderr)
        sys.exit(1)

    print("Dispositivos Basilisk (evdev):")
    fds: dict[int, tuple[str, str, str]] = {}
    for path, name in nodes:
        try:
            fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
            fds[fd] = ("evdev", path, name)
            print(f"  {path}  ({name})")
        except OSError as e:
            print(f"  {path}  ERROR: {e}")

    print("\nHidraw (estado DPI / informes brutos):")
    for path in hidraws:
        try:
            fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
            fds[fd] = ("hidraw", path, "")
            print(f"  {path}")
        except OSError as e:
            print(f"  {path}  ERROR: {e}")

    print()
    print("Pulsa UNO A UNO los botones que no se resaltan en Synapsex.")
    print("Orden sugerido: Perfil M1 → DPI → HyperShift → Lateral AI → otros")
    print("Si Perfil/DPI/HS/AI no aparecen aquí, es normal: viven en el firmware.")
    print("Ctrl+C para terminar.\n")
    print(f"{'dev':10} {'type':6} detail")
    print("-" * 64)

    fmt = "llHHi"
    esz = struct.calcsize(fmt)
    try:
        while fds:
            ready, _, _ = select.select(list(fds), [], [], 0.5)
            for fd in ready:
                kind, path, name = fds[fd]
                short = path.rsplit("/", 1)[-1]
                try:
                    data = os.read(fd, 64 * 24 if kind == "evdev" else 64)
                except BlockingIOError:
                    continue
                if kind == "hidraw":
                    if len(data) >= 3 and data[0] == 0x05 and data[1] == 0x56:
                        print(
                            f"{short:10} HIDRAW DPI_STAGE={data[2]}  hex={data[:8].hex()}"
                        )
                    elif any(b != 0 for b in data[1:]):
                        print(f"{short:10} HIDRAW L={len(data)} {data.hex()}")
                    sys.stdout.flush()
                    continue
                for off in range(0, len(data), esz):
                    if off + esz > len(data):
                        break
                    _s, _u, typ, code, val = struct.unpack_from(fmt, data, off)
                    if typ == EV_KEY and val == 1:
                        label = BTN.get(code) or KEY_NAMED.get(code) or f"KEY_{code}"
                        print(f"{short:10} KEY    {code:5} 0x{code:03x}  {label}  [{name}]")
                        sys.stdout.flush()
                    elif typ == EV_REL and code in REL and val != 0:
                        print(f"{short:10} REL    {code:5} 0x{code:03x}  {REL[code]}={val}")
                        sys.stdout.flush()
                    elif typ == EV_MSC and val != 0:
                        print(f"{short:10} MSC    {code:5} 0x{code:03x}  val={val}")
                        sys.stdout.flush()
    except KeyboardInterrupt:
        print("\nListo — pega esta salida en el chat.")
    finally:
        for fd in fds:
            os.close(fd)


if __name__ == "__main__":
    main()
