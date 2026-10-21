#!/usr/bin/env python3
"""Writes htmlparse/entity_data.ri from html5ever's table (web_atoms 0.1.3,
the WHATWG list as markup5ever 0.35 has it):

    python3 tools/entities.py ~/.cargo/registry/src/*/web_atoms-0.1.3/entities.rs
"""
import re
import sys

rows = re.findall(r'\("&([^"]+)", (\d+), (\d+)\)', open(sys.argv[1]).read())
assert len(rows) == 2231, len(rows)
lines = [f"{name} {a} {b}" for name, a, b in rows]
out = open("htmlparse/entity_data.ri", "w")
out.write("""# The named character references, from html5ever's table (web_atoms
# 0.1.3): a name, with its `;` where it has one, and one or two code
# points. Written by tools/entities.py; do not edit.
module htmlparse/entity_data

pub ENTITY_DATA = <<-DATA
""")
for line in lines:
    out.write("  " + line + "\n")
out.write("  DATA\n")
