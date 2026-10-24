#!/usr/bin/env python3
"""Writes htmlparse/tags.ri: the element names the tree builder decides by.

    python3 tools/tags.py
"""
NAMES = """
a address applet area article aside b base basefont bgsound big blockquote
body br button caption center code col colgroup dd details dialog dir div dl
dt em embed fieldset figcaption figure font footer form frame frameset h1 h2
h3 h4 h5 h6 head header hgroup hr html i iframe image img input isindex keygen
li link listing main marquee math menu meta nav nobr noembed noframes noscript
object ol optgroup option p param plaintext pre rb rp rt rtc ruby s script
search section select small source span strike strong style sub summary sup
svg table tbody td template textarea tfoot th thead title tr track tt u ul var
wbr xmp mi mo mn ms mtext annotation-xml mglyph malignmark foreignObject desc
""".split()
assert len(NAMES) == len(set(NAMES))


def member(name):
    parts = name.replace("-", " ").split()
    return "".join(p[0].upper() + p[1:] for p in parts)


out = open("htmlparse/tags.ri", "w")
out.write('''# The element names the tree builder decides by, as numbers: `tag_of`
# answers `Tag::Other` for every other name. Names are compared exactly,
# so an SVG element's adjusted name (`foreignObject`) has its own. Written
# by tools/tags.py; do not edit.
module htmlparse/tags

import std/int

pub enum Tag : UInt16
  Other
''')
for n in NAMES:
    out.write(f"  {member(n)}\n")
out.write("end\n\nNAMES = [\n")
for n in NAMES:
    out.write(f'  "{n}",\n')
out.write(''']

# Open addressing over the names, FNV-1a, so a name on the tape is looked
# up without a copy of it.
class Table
  @keys : Array(String)
  @values : Array(UInt16)

  def initialize
    @keys = Array(String).new(512, "")
    @values = Array(UInt16).new(512, 0_u16)
    NAMES.each_with_index do |name, i|
      slot = (hash(Str.view(name)) & 511_u32).to_i32
      while @values[slot] != 0_u16
        slot = (slot + 1) & 511
      end
      @keys[slot] = name
      @values[slot] = (i + 1).to_u16
    end
  end

  def hash(key : Str) : UInt32
    h = 2166136261_u32
    key.each_byte { |b| h = (h ^ b.to_u32) &* 16777619_u32 }
    h
  end

  def lookup(name : Str) : Tag
    slot = (hash(name) & 511_u32).to_i32
    while true
      value = @values[slot]
      return Tag::Other if value == 0_u16
      return Tag.new(value) if name == @keys[slot]
      slot = (slot + 1) & 511
    end
    Tag::Other
  end
end

TABLE = Table.new

pub def tag_of(name : Str) : Tag
  TABLE.lookup(name)
end

# The name of a `Tag` other than `Other`.
pub def tag_name(tag : Tag) : String
  NAMES[tag.value.to_i32 - 1]
end
''')
