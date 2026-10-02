# Fixtures

Both fonts are synthesized from scratch by `make.py` (deterministic: run it to regenerate), so
nothing in them is licensed from anyone. Every glyph is a rectangle.

- `blocks.ttf`: family `Anyview Blocks`, style `Regular`. Maps A-Z, a-z, 0-9, space and `&?!@`, each
  to a rectangle of its own size, with a Unicode cmap. All three specimen lines are drawn from it.
- `circled.ttf`: family `Anyview Circled`. Maps only U+2460 to U+2469, so none of the specimen's
  sample letters: the specimen shows the characters the font does have.
