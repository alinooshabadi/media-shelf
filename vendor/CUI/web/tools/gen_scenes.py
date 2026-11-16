#!/usr/bin/env python3
"""Copies the scene table out of tests/component_shots.c (it lives inside
main) so the web module can serve the same scenes.
    gen_scenes.py <repo-root> <out.inc>"""
import re, sys

root, out = sys.argv[1], sys.argv[2]
src = open(f"{root}/tests/component_shots.c", encoding="utf-8").read()
m = re.search(r"static const scene scenes\[\] = \{(.*?)\n    \};", src, re.S)
if not m:
    sys.exit("gen_scenes: scene table not found in tests/component_shots.c")
body = m.group(1).strip()
open(out, "w").write("/* generated from tests/component_shots.c, do not edit */\n" + body + "\n")
print(f"  scenes: {len(re.findall(r'{ \"', body))} from component_shots.c")
