#!/usr/bin/env python3
"""
keymap_docs.py - draw the Parix 52 keymap from config/keyboard.toml.

    python3 scripts/keymap_docs.py [--toml config/keyboard.toml]
                                   [--vial config/vial.json]
                                   [--out docs/keymap.svg]

The physical key positions come from config/vial.json (the same KLE layout
Vial shows), the layers and combos from keyboard.toml, so the drawing can
never disagree with the firmware. Rendering is done by keymap-drawer
(`pip install keymap-drawer==0.23.0`), which must be on PATH as `keymap`.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    sys.exit("Python 3.11+ is required (tomllib)")

KEYS = 52

# Keycode -> label on the key
KEYCODE_DISPLAY: dict[str, str] = {
    **{f"Kc{i}": str(i) for i in range(10)},
    **{f"F{i}": f"F{i}" for i in range(1, 13)},
    "Comma": ",", "Dot": ".", "Slash": "/", "Quote": "'", "Grave": "`",
    "Semicolon": ";", "Equal": "=", "Minus": "-", "Backslash": "\\",
    "LeftBracket": "[", "RightBracket": "]",
    "Left": "←", "Down": "↓", "Up": "↑", "Right": "→",
    "Home": "Home", "End": "End", "PageUp": "PgUp", "PageDown": "PgDn",
    "Insert": "Ins", "CapsLock": "Caps", "CapsWordToggle": "Caps Word",
    "Enter": "Enter", "Backspace": "Bspc", "Delete": "Del", "Escape": "Esc",
    "Tab": "Tab", "Menu": "Menu", "PrintScreen": "PrtSc",
    "ScrollLock": "ScrLk", "Pause": "Pause", "Space": "Space",
    "AudioVolUp": "Vol+", "AudioVolDown": "Vol-", "AudioMute": "Mute",
    "MediaPlayPause": "Play", "MediaStop": "Stop",
    "MediaPrevTrack": "Prev", "MediaNextTrack": "Next",
    "BrightnessUp": "Bri+", "BrightnessDown": "Bri-",
    "MouseLeft": "Ptr ←", "MouseDown": "Ptr ↓", "MouseUp": "Ptr ↑", "MouseRight": "Ptr →",
    "MouseWheelLeft": "Whl ←", "MouseWheelDown": "Whl ↓",
    "MouseWheelUp": "Whl ↑", "MouseWheelRight": "Whl →",
    "MouseBtn1": "L click", "MouseBtn2": "R click", "MouseBtn3": "M click",
    # ble_profiles_num = 4 gives these User keys their meaning
    "User0": "BT 1", "User1": "BT 2", "User2": "BT 3", "User3": "BT 4",
    "User4": "BT next", "User5": "BT prev", "User6": "BT clear", "User7": "USB/BT",
    "User10": "OLED -", "User11": "OLED +",
    "User12": "Base light", "User13": "Light -", "User14": "Light +", "User15": "Base colour",
    "LGui": "Cmd", "RGui": "Cmd", "LAlt": "Opt", "RAlt": "Opt",
    "LCtrl": "Ctrl", "RCtrl": "Ctrl", "LShift": "Shift", "RShift": "Shift",
    "No": "", "_______": "",
}
MOD_SYM = {"LGui": "⌘", "RGui": "⌘", "LAlt": "⌥", "RAlt": "⌥",
           "LCtrl": "⌃", "RCtrl": "⌃", "LShift": "⇧", "RShift": "⇧"}
SHIFTED_DISPLAY = {
    "Kc1": "!", "Kc2": "@", "Kc3": "#", "Kc4": "$", "Kc5": "%", "Kc6": "^",
    "Kc7": "&", "Kc8": "*", "Kc9": "(", "Kc0": ")", "Minus": "_", "Equal": "+",
    "LeftBracket": "{", "RightBracket": "}", "Backslash": "|", "Semicolon": ":",
    "Grave": "~", "Quote": '"', "Slash": "?", "Dot": ">", "Comma": "<",
}


def tokenize(keys: str) -> list[str]:
    """Split on whitespace outside parentheses: 'MT(A, LGui, HRM) G' -> 2 tokens."""
    out, buf, depth = [], [], 0
    for ch in keys:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch.isspace() and depth == 0:
            if buf:
                out.append("".join(buf))
                buf = []
        else:
            buf.append(ch)
    if buf:
        out.append("".join(buf))
    return out


def labels(token: str, layer_names: list[str]) -> tuple[str, str | None]:
    """(tap label, hold label or None) for one keyboard.toml key token."""
    m = re.fullmatch(r"MT\(\s*(\w+)\s*,\s*(\w+)(?:\s*,\s*\w+)?\s*\)", token)
    if m:
        return KEYCODE_DISPLAY.get(m[1], m[1]), KEYCODE_DISPLAY.get(m[2], m[2])
    m = re.fullmatch(r"LT\(\s*(\d+)\s*,\s*(\w+)(?:\s*,\s*\w+)?\s*\)", token)
    if m:
        n = int(m[1])
        return KEYCODE_DISPLAY.get(m[2], m[2]), layer_names[n] if n < len(layer_names) else f"L{n}"
    m = re.fullmatch(r"TG\(\s*(\d+)\s*\)", token)
    if m:
        n = int(m[1])
        return f"{layer_names[n] if n < len(layer_names) else n} on/off", None
    m = re.fullmatch(r"WM\(\s*(\w+)\s*,\s*(.+?)\s*\)", token)
    if m:
        mods = [s.strip() for s in m[2].split("|")]
        sym = "".join(MOD_SYM.get(x, x) for x in mods)
        if m[1] == "No":
            return ("Hyper" if len(mods) == 4 else sym), None
        return sym + KEYCODE_DISPLAY.get(m[1], m[1]), None
    m = re.fullmatch(r"SHIFTED\(\s*(\w+)\s*\)", token)
    if m:
        return SHIFTED_DISPLAY.get(m[1], KEYCODE_DISPLAY.get(m[1], m[1])), None
    return KEYCODE_DISPLAY.get(token, token), None


def load(toml_path: Path) -> tuple[list[dict], list[dict], list[str]]:
    data = tomllib.loads(toml_path.read_text())
    # RMK 0.9 moved the layers under [keymap] and renamed matrix_map to map;
    # the older shape is still read so the script works on either.
    raw_layers = data.get("keymap", {}).get("layer") or data["layer"]
    names = [l["name"] for l in raw_layers]
    layers = []
    for layer in raw_layers:
        tokens = tokenize(layer["keys"])
        if len(tokens) != KEYS:
            sys.exit(f"layer {layer['name']}: {len(tokens)} keys, expected {KEYS}")
        layers.append({"name": layer["name"], "tokens": tokens,
                       "keys": [labels(t, names) for t in tokens]})
    matrix = [tuple(int(n) for n in re.findall(r"\d+", t))
              for t in tokenize(data["layout"].get("map") or data["layout"]["matrix_map"])]
    combos = data.get("behavior", {}).get("combo", {}).get("combos", [])
    return layers, combos, [f"{r},{c}" for r, c in matrix]


def physical_layout(vial_path: Path, order: list[str]) -> dict:
    """QMK info.json-style layout from the Vial KLE, in keyboard.toml key order."""
    kle = json.loads(vial_path.read_text())["layouts"]["keymap"]
    pos: dict[str, dict] = {}
    for row in kle:
        props: dict = {}
        for item in row:
            if isinstance(item, dict):
                props = {**props, **item}
                continue
            # KLE: the key's corner sits at (rx + x, ry + y) in a frame
            # rotated r degrees about (rx, ry).
            rx, ry = props.get("rx", 0.0), props.get("ry", 0.0)
            key = {"x": rx + props.get("x", 0.0), "y": ry + props.get("y", 0.0),
                   "w": props.get("w", 1.0), "h": props.get("h", 1.0)}
            if props.get("r"):
                key.update({"r": props["r"], "rx": rx, "ry": ry})
            pos[item] = key
            props = {k: v for k, v in props.items() if k in ("r", "rx", "ry")}
    missing = [m for m in order if m not in pos]
    if missing:
        sys.exit(f"vial.json has no key for matrix position(s) {missing}")
    return {"keyboard_name": "Parix 52",
            "layouts": {"LAYOUT": {"layout": [pos[m] for m in order]}}}


def yaml_str(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def build_yaml(layers: list[dict], combos: list[dict], info_json: str) -> str:
    lines = ["layout:", f"  qmk_info_json: {yaml_str(info_json)}",
             "  layout_name: LAYOUT", "", "layers:"]
    for layer in layers:
        lines.append(f"  {layer['name']}:")
        for tap, hold in layer["keys"]:
            if hold is None:
                lines.append(f"    - {yaml_str(tap)}")
            else:
                lines.append(f"    - {{t: {yaml_str(tap)}, h: {yaml_str(hold)}}}")
        lines.append("")
    base = layers[0]["tokens"]
    names = [l["name"] for l in layers]
    drawn = []
    for combo in combos:
        try:
            idx = [base.index(a) for a in combo["actions"]]
        except ValueError:
            continue  # a combo on a key that is not on BASE: not drawn
        out, _ = labels(combo["output"], names)
        drawn.append(f"  - {{p: {idx}, k: {yaml_str(out)}, l: [{names[0]}]}}")
    if drawn:
        lines += ["combos:"] + drawn + [""]
    return "\n".join(lines)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--toml", default="config/keyboard.toml")
    ap.add_argument("--vial", default="config/vial.json")
    ap.add_argument("--out", default="docs/keymap.svg")
    a = ap.parse_args()

    layers, combos, order = load(Path(a.toml))
    info = physical_layout(Path(a.vial), order)
    with tempfile.TemporaryDirectory() as tmp:
        info_path = os.path.join(tmp, "info.json")
        Path(info_path).write_text(json.dumps(info))
        yaml_path = os.path.join(tmp, "keymap.yaml")
        Path(yaml_path).write_text(build_yaml(layers, combos, info_path))
        Path(a.out).parent.mkdir(parents=True, exist_ok=True)
        r = subprocess.run(["keymap", "draw", "-o", a.out, yaml_path],
                           capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"keymap draw failed:\n{r.stderr}")
    size = Path(a.out).stat().st_size
    print(f"{a.out}: {len(layers)} layers, {len(combos)} combos, {size:,} bytes")


if __name__ == "__main__":
    main()
