#!/usr/bin/env python3
"""Rename an upstream XCL tree to Mandy, mechanically and repeatably.

Usage: python rename_upstream.py <tree>

Run on a clean checkout of upstream. The same rules applied to every upstream
snapshot give trees that merge cleanly into Mandy's main branch.

Rules (case-sensitive, so console API names such as XClosedCaption are kept):
  xcl -> mandy, Xcl -> Mandy, XCL -> MANDY
  unless a letter comes right before (exclude, EXCLUSIVE), except the n of an
  escaped newline (backslash, n, xcl).
Kept as they are: links to the upstream repository and its Discord, the
LICENSE file, and anything under .git.
"""
import os
import re
import subprocess
import sys

NUL = bytes([0])
BACKSLASH_N = bytes([0x5C]) + b"n"

KEEP_FILES = {"LICENSE"}
# Source files are text even when they hold a NUL byte in a literal.
TEXT_EXT = {
    ".rs", ".md", ".toml", ".yml", ".yaml", ".ps1", ".py", ".hlsl", ".txt",
    ".json", ".lock",
}
PROTECT = [b"ivall/xcl", b"discord.gg/xcl"]
PAIRS = [(b"xcl", b"mandy"), (b"Xcl", b"Mandy"), (b"XCL", b"MANDY")]
# Not after a letter, or after the "n" of a backslash-n escape.
PATTERNS = [
    (
        re.compile(
            b"(?:(?<![A-Za-z])|(?<=" + re.escape(BACKSLASH_N) + b"))" + re.escape(old)
        ),
        new,
    )
    for old, new in PAIRS
]
# Literals of a fixed size: the longer name must not change their length.
# Before the rename: the 8-byte magic of the translated-shader cache.
PRE = [(re.compile(b"XCLTRAN([0-9])"), b"MDYTRAN" + bytes([0x5C]) + b"1")]
# After the rename: the 32-byte gamepad device id loses two padding NULs.
POST = [(re.compile(b"(MANDY gamepad [0-9])" + re.escape(NUL + NUL)), bytes([0x5C]) + b"1")]


def rename_bytes(data: bytes) -> bytes:
    for pattern, new in PRE:
        data = pattern.sub(new, data)
    held = {}
    for i, text in enumerate(PROTECT):
        token = bytes([1]) + b"KEEP%d" % i + bytes([1])
        held[token] = text
        data = data.replace(text, token)
    for pattern, new in PATTERNS:
        data = pattern.sub(new, data)
    for token, text in held.items():
        data = data.replace(token, text)
    for pattern, new in POST:
        data = pattern.sub(new, data)
    return data


def sort_lock(data: bytes) -> bytes:
    """Cargo.lock lists packages by name: put the renamed ones back in order."""
    eol = b"\r\n" if b"\r\n" in data else b"\n"
    marker = eol + b"[[package]]" + eol
    head, *blocks = data.split(marker)
    if not blocks:
        return data
    tail = b""
    last = blocks[-1]
    cut = last.find(eol + eol + b"[")  # a trailing [metadata] or [[patch]] table
    if cut != -1:
        blocks[-1], tail = last[:cut], last[cut:]
    blocks = [b.rstrip(b"\r\n") for b in blocks]
    blocks.sort(key=lambda b: b.split(eol)[:2])
    return head.rstrip(b"\r\n") + eol + eol + eol.join(
        b"[[package]]" + eol + b + eol for b in blocks
    ) + tail.lstrip(b"\r\n")


def tracked(tree):
    out = subprocess.run(
        ["git", "-C", tree, "ls-files", "-z"], check=True, capture_output=True
    ).stdout
    return [p.decode("utf-8") for p in out.split(NUL) if p]


def main():
    tree = sys.argv[1]
    changed = moved = 0
    files = tracked(tree)
    for rel in files:
        path = os.path.join(tree, rel)
        if os.path.basename(rel) in KEEP_FILES or os.path.islink(path):
            continue
        with open(path, "rb") as f:
            data = f.read()
        if NUL in data and os.path.splitext(rel)[1] not in TEXT_EXT:  # binary
            continue
        new = rename_bytes(data)
        if os.path.basename(rel) == "Cargo.lock":
            new = sort_lock(new)
        if new != data:
            with open(path, "wb") as f:
                f.write(new)
            changed += 1
    for rel in files:
        new_rel = rename_bytes(rel.encode("utf-8")).decode("utf-8")
        if new_rel != rel:
            dst = os.path.join(tree, new_rel)
            os.makedirs(os.path.dirname(dst) or tree, exist_ok=True)
            subprocess.run(["git", "-C", tree, "mv", "-k", rel, new_rel], check=True)
            moved += 1
    print(f"{changed} files edited, {moved} paths moved")


if __name__ == "__main__":
    main()
