#!/usr/bin/env python3
"""Capture *all* Basilisk V4 Pro host-visible signals (evdev + hidraw).

Not limited to evtest: opens every matching /dev/input/event* and /dev/hidraw*,
logs keys, relative axes (except continuous pointer X/Y spam), MSC, and every
non-trivial hidraw interrupt report.

Usage:
  python3 scripts/probe-all-signals.py

  Then press ONE button at a time, wait ~1s between presses:
    1) HyperShift (gatillo separado)
    2) Lateral AI (el del frente de los 3)
    3) Adelante
    4) Atrás
    5) DPI
    6) Perfil M1
  Ctrl+C → resumen.

Optional:
  SYNAPSEX_PROBE_MOTION=1  also log REL_X/Y (very noisy)
  SYNAPSEX_PROBE_SECS=60   auto-stop after N seconds
"""

from __future__ import annotations

import glob
import os
import select
import struct
import sys
import time
from collections import Counter, defaultdict

EV_SYN = 0
EV_KEY = 1
EV_REL = 2
EV_MSC = 4
EV_ABS = 3
EV_SW = 5

BTN = {
    0x100: "BTN_MISC",
    0x110: "BTN_LEFT",
    0x111: "BTN_RIGHT",
    0x112: "BTN_MIDDLE",
    0x113: "BTN_SIDE",
    0x114: "BTN_EXTRA",
    0x115: "BTN_FORWARD",
    0x116: "BTN_BACK",
    0x117: "BTN_TASK",
}
for i in range(0x118, 0x152):
    BTN.setdefault(i, f"BTN_{i:#x}")

KEY_NAMED = {
    1: "ESC",
    28: "ENTER",
    57: "SPACE",
    **{59 + i: f"F{i+1}" for i in range(10)},
    87: "F11",
    88: "F12",
    **{183 + i: f"F{13+i}" for i in range(12)},
}

REL = {
    0: "X",
    1: "Y",
    6: "HWHEEL",
    7: "DIAL",
    8: "WHEEL",
    9: "MISC",
    11: "WHEEL_HI_RES",
    12: "HWHEEL_HI_RES",
}

# Mouse report ids that are usually continuous motion (still logged if button bits change).
MOTION_REPORT_IDS = {0x01, 0x04}


def basilisk_evdev() -> list[tuple[str, str]]:
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


def basilisk_hidraw() -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []
    for uevent in sorted(glob.glob("/sys/class/hidraw/hidraw*/device/uevent")):
        text = open(uevent).read()
        low = text.lower()
        if "00001532:" not in low:
            continue
        if ":000000e3" not in low and ":000000e2" not in low:
            continue
        name = uevent.split("/")[4]
        iface = "?"
        try:
            iface = open(
                f"/sys/class/hidraw/{name}/device/../bInterfaceNumber"
            ).read().strip()
        except OSError:
            pass
        out.append((f"/dev/{name}", f"iface{iface}"))
    return out


def key_label(code: int) -> str:
    return BTN.get(code) or KEY_NAMED.get(code) or f"KEY_{code}"


def interesting_hidraw(data: bytes, last_btn_state: dict[str, bytes]) -> tuple[bool, str]:
    """Return (log?, reason). Always log non-motion; for motion reports, log on button-byte change."""
    if not data:
        return False, ""
    rid = data[0]
    path_key = ""  # filled by caller via last_btn_state key
    # Status / vendor notifies (DPI stage etc.)
    if rid == 0x05 and len(data) >= 3 and data[1] == 0x56:
        return True, f"DPI_STAGE={data[2]}"
    if rid not in MOTION_REPORT_IDS:
        if any(b != 0 for b in data[1:]):
            return True, "vendor/other"
        return False, ""
    # Typical boot mouse: rid, buttons, x, y, wheel…
    # Keep first 3 payload bytes as "button-ish" fingerprint (buttons + low axes).
    fingerprint = data[:8]
    return True, "motion-frame"  # filtered further by caller using fingerprint


def main() -> None:
    log_motion = os.environ.get("SYNAPSEX_PROBE_MOTION", "") == "1"
    auto_secs = float(os.environ.get("SYNAPSEX_PROBE_SECS", "0") or "0")

    evdevs = basilisk_evdev()
    hidraws = basilisk_hidraw()
    if not evdevs and not hidraws:
        print("No Basilisk nodes (VID 1532, PID 00e2/00e3).", file=sys.stderr)
        sys.exit(1)

    print("=== Synapsex probe-all-signals ===")
    print("Evdev:")
    fds: dict[int, tuple[str, str, str]] = {}
    for path, name in evdevs:
        try:
            fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
            fds[fd] = ("evdev", path, name)
            print(f"  {path}  ({name})")
        except OSError as e:
            print(f"  {path}  ERROR: {e}")

    print("Hidraw:")
    for path, label in hidraws:
        try:
            fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
            fds[fd] = ("hidraw", path, label)
            print(f"  {path}  ({label})")
        except OSError as e:
            print(f"  {path}  ERROR: {e}")

    print()
    print("Pulsa UN botón a la vez (~1s entre pulsos):")
    print("  1 HyperShift  2 Lateral AI  3 Adelante  4 Atrás  5 DPI  6 Perfil M1")
    print("Ctrl+C para resumen.\n")
    print(f"{'t':>8}  {'dev':10}  detail")
    print("-" * 72)

    fmt = "llHHi"
    esz = struct.calcsize(fmt)
    t0 = time.monotonic()
    counts: Counter[str] = Counter()
    by_dev: dict[str, Counter[str]] = defaultdict(Counter)
    last_hid_fp: dict[str, bytes] = {}
    last_key_down: dict[tuple[str, int], float] = {}

    def stamp() -> str:
        return f"{time.monotonic() - t0:7.3f}s"

    try:
        end = t0 + auto_secs if auto_secs > 0 else None
        while fds:
            if end is not None and time.monotonic() >= end:
                print("\n(auto-stop)")
                break
            ready, _, _ = select.select(list(fds), [], [], 0.25)
            for fd in ready:
                kind, path, meta = fds[fd]
                short = path.rsplit("/", 1)[-1]
                try:
                    data = os.read(fd, 4096)
                except BlockingIOError:
                    continue
                if not data:
                    continue

                if kind == "hidraw":
                    # May contain multiple reports concatenated; split heuristically by known lens.
                    off = 0
                    while off < len(data):
                        # Prefer fixed 16-byte vendor chunks when present; else take rest.
                        chunk = data[off : off + 16]
                        if len(chunk) < 3:
                            break
                        rid = chunk[0]
                        # Variable: if looks like short mouse report, take 8
                        if rid in MOTION_REPORT_IDS and len(data) - off >= 8:
                            # Don't assume length — dump up to 16, but fingerprint first 8
                            take = min(16, len(data) - off)
                            chunk = data[off : off + take]
                            off += take
                        else:
                            take = min(16, len(data) - off)
                            chunk = data[off : off + take]
                            off += take

                        if rid == 0x05 and len(chunk) >= 3:
                            marker = chunk[1]
                            val = chunk[2]
                            if marker == 0x56:
                                msg = f"HID DPI_STAGE={val}  {chunk.hex()}"
                                counts["hidraw:dpi"] += 1
                                by_dev[short]["dpi"] += 1
                            elif marker == 0x39:
                                edge = "DOWN" if val else "UP"
                                msg = f"HID HYPERSHIFT {edge}  {chunk.hex()}"
                                counts[f"hidraw:hypershift:{edge}"] += 1
                                by_dev[short][f"hypershift:{edge}"] += 1
                            elif marker == 0x80:
                                edge = "DOWN" if val else "UP"
                                msg = f"HID SIDE_AI {edge}  {chunk.hex()}"
                                counts[f"hidraw:side_ai:{edge}"] += 1
                                by_dev[short][f"side_ai:{edge}"] += 1
                            elif marker == 0x0E:
                                msg = f"HID PROFILE_M1 val={val}  {chunk.hex()}"
                                counts["hidraw:profile"] += 1
                                by_dev[short]["profile"] += 1
                            else:
                                msg = None
                            if msg:
                                print(f"{stamp()}  {short:10}  {msg}")
                                sys.stdout.flush()
                                continue

                        fp = bytes(chunk[:8])
                        prev = last_hid_fp.get(f"{short}:{rid}")
                        if rid in MOTION_REPORT_IDS:
                            # Log only when button-ish bytes (index 1 typically) change.
                            btn = chunk[1] if len(chunk) > 1 else 0
                            prev_btn = prev[1] if prev and len(prev) > 1 else None
                            if prev is not None and btn == prev_btn and not log_motion:
                                last_hid_fp[f"{short}:{rid}"] = fp
                                continue
                            if prev is not None and fp == prev and not log_motion:
                                continue
                            last_hid_fp[f"{short}:{rid}"] = fp
                            # Skip pure motion (btn byte 0 and only xy changed) unless motion logging
                            if btn == 0 and (prev_btn in (None, 0)) and not log_motion:
                                continue
                            msg = f"HID rid={rid:#04x} btn={btn:#04x}  {chunk.hex()}"
                            print(f"{stamp()}  {short:10}  {msg}")
                            counts[f"hidraw:rid{rid:02x}"] += 1
                            by_dev[short][f"btn={btn:#04x}"] += 1
                            sys.stdout.flush()
                        else:
                            if any(b != 0 for b in chunk[1:]):
                                if prev == fp:
                                    continue
                                last_hid_fp[f"{short}:{rid}"] = fp
                                msg = f"HID rid={rid:#04x} L={len(chunk)}  {chunk.hex()}"
                                print(f"{stamp()}  {short:10}  {msg}")
                                counts[f"hidraw:rid{rid:02x}"] += 1
                                by_dev[short][f"rid={rid:#04x}"] += 1
                                sys.stdout.flush()
                    continue

                # evdev
                for i in range(0, len(data), esz):
                    if i + esz > len(data):
                        break
                    _s, _u, typ, code, val = struct.unpack_from(fmt, data, i)
                    if typ == EV_SYN:
                        continue
                    if typ == EV_KEY:
                        edge = "DOWN" if val == 1 else "UP" if val == 0 else f"REP({val})"
                        if val == 1:
                            last_key_down[(short, code)] = time.monotonic()
                        label = key_label(code)
                        msg = f"KEY {edge:4} {code:4} 0x{code:03x}  {label}  [{meta}]"
                        print(f"{stamp()}  {short:10}  {msg}")
                        counts[f"key:{label}:{edge}"] += 1
                        by_dev[short][f"{label}:{edge}"] += 1
                        sys.stdout.flush()
                    elif typ == EV_REL:
                        name = REL.get(code, f"REL_{code}")
                        if name in ("X", "Y") and not log_motion:
                            continue
                        if val == 0:
                            continue
                        msg = f"REL {name}={val}"
                        print(f"{stamp()}  {short:10}  {msg}")
                        counts[f"rel:{name}"] += 1
                        by_dev[short][name] += 1
                        sys.stdout.flush()
                    elif typ == EV_MSC and val != 0:
                        msg = f"MSC code={code} val={val} (0x{val:x})"
                        print(f"{stamp()}  {short:10}  {msg}")
                        counts["msc"] += 1
                        by_dev[short]["msc"] += 1
                        sys.stdout.flush()
                    elif typ == EV_ABS:
                        msg = f"ABS code={code} val={val}"
                        print(f"{stamp()}  {short:10}  {msg}")
                        counts["abs"] += 1
                        sys.stdout.flush()
                    elif typ == EV_SW:
                        msg = f"SW code={code} val={val}"
                        print(f"{stamp()}  {short:10}  {msg}")
                        counts["sw"] += 1
                        sys.stdout.flush()
                    else:
                        msg = f"type={typ} code={code} val={val}"
                        print(f"{stamp()}  {short:10}  {msg}")
                        counts[f"other:{typ}"] += 1
                        sys.stdout.flush()
    except KeyboardInterrupt:
        print("\n--- resumen ---")
    finally:
        for fd in fds:
            os.close(fd)

    print(f"\nDuración: {time.monotonic() - t0:.1f}s")
    print("Contadores globales:")
    for k, v in counts.most_common(40):
        print(f"  {v:5}  {k}")
    print("\nPor dispositivo:")
    for dev, c in sorted(by_dev.items()):
        print(f"  [{dev}]")
        for k, v in c.most_common(20):
            print(f"    {v:5}  {k}")
    print(
        "\nSi HyperShift / Lateral AI no aparecen arriba, el firmware no los"
        " envía al host (solo DPI=05 56 y clics clásicos)."
    )
    print("Pega esta salida en el chat.")


if __name__ == "__main__":
    main()
