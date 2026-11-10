#!/usr/bin/env python3
"""Draws src/game/scene/surface-5-sides.png, surface picture 5's sides in a 16:9 frame
(docs/widescreen.md), from shapes alone: crystals, lines, a bush, bands of ground. Deterministic,
standard library only. Coordinates are the picture's (x 0-255, lines 0-239); the art covers
x -88 to 343. The points where a shape meets the picture's edge were measured on the picture.

Run from the repository root:
    python3 tools/art/surface-5-sides.py [--crystals N] [--bush N] [--out FILE]
N picks one of the designs below (1-5); the defaults are the chosen ones.
"""

import argparse
import math
import struct
import zlib

LEFT, WIDTH, HEIGHT, TOP = 88, 432, 240, 46
GROUND_END = 222  # the picture's ground ends at line 221
HORIZON = (102, 126)  # the gold line and pink band, rows 13-15

# Colours by name: (palette, value); "W" (white) takes the cell's palette.
GOLD, PURPLE, PINK = 0, 1, 2
INK = {
    ".": (0, 0),
    "G": (GOLD, 2), "d": (GOLD, 3),
    "m": (PURPLE, 2), "u": (PURPLE, 3),
    "p": (PINK, 2), "q": (PINK, 3),
}

canvas = {}


def put(x, y, c):
    if -LEFT <= x < WIDTH - LEFT and TOP <= y < HEIGHT and not 0 <= x < 256:
        canvas[(x, y)] = c


def get(x, y):
    return canvas.get((x, y))


def hash01(*k):
    """A fixed pseudo-random number in [0, 1) for the integers k."""
    h = 2166136261
    for v in k:
        h = ((h ^ (int(v) & 0xFFFFFFFF)) * 16777619) & 0xFFFFFFFF
    h ^= h >> 13
    h = (h * 0x5BD1E995) & 0xFFFFFFFF
    h ^= h >> 15
    return h / 2**32


def points(x0, y0, x1, y1):
    n = max(abs(round(x1) - round(x0)), abs(round(y1) - round(y0)), 1)
    for i in range(n + 1):
        t = i / n
        yield i, round(x0 + (x1 - x0) * t), round(y0 + (y1 - y0) * t)


def line(x0, y0, x1, y1, c="G"):
    for _, x, y in points(x0, y0, x1, y1):
        put(x, y, c)


def edge(a, b, width=1, inner=(0, 1), c="G", shade="d"):
    """A crystal's edge from a to b: `width` 1 is a gold line; 2 adds dark gold on the side
    `inner` points to; 3 adds a second, checked row of it."""
    for i, x, y in points(*a, *b):
        put(x, y, c)
        if width >= 2:
            put(x + inner[0], y + inner[1], shade)
        if width >= 3 and i % 2 == 0:
            put(x + 2 * inner[0], y + 2 * inner[1], shade)


def ray(x0, y0, x1, y1):
    """A ray of the cluster: gold where it leaves the picture, fading out to dotted dark."""
    for i, x, y in points(x0, y0, x1, y1):
        t = i / max(1, max(abs(x1 - x0), abs(y1 - y0)))
        if t < 0.35:
            put(x, y, "G" if i % 3 else "d")
        elif t < 0.7 or i % 2 == 0:
            put(x, y, "d")


def inside(poly, x, y):
    c = False
    for i in range(len(poly)):
        (ax, ay), (bx, by) = poly[i], poly[i - 1]
        if (ay > y) != (by > y) and x < (bx - ax) * (y - ay) / (by - ay) + ax:
            c = not c
    return c


def seg_dist(px, py, a, b):
    (ax, ay), (bx, by) = a, b
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy or 1)))
    return math.hypot(px - ax - t * dx, py - ay - t * dy)


def shade_at(x, y, d, tone):
    """The picture's shading: `tone` 0 (dark face) to 3 (lit face) sets how far from the lit
    edge (distance d) the checks of gold and dark gold, then dark checks, then specks reach."""
    reach = (1.2, 2.5, 4.0, 6.5)[tone]
    k = d / reach
    check = (x + y) % 2
    if k < 0.3:
        return "G" if check else "d"
    if k < 0.55:
        return "d" if check else ("G" if (x // 2 + y) % 3 == 0 else "d")
    if k < 1.0:
        return "d" if check else "."
    if k < 1.5:
        return "d" if check and y % 2 else "."
    if k < 2.2:
        return "d" if check and y % 4 == 1 and x % 4 == 1 else "."
    return "."


def face(poly, lit, tone):
    xs = [p[0] for p in poly]
    ys = [p[1] for p in poly]
    for y in range(math.floor(min(ys)), math.ceil(max(ys)) + 1):
        for x in range(math.floor(min(xs)), math.ceil(max(xs)) + 1):
            if inside(poly, x + 0.5, y + 0.5):
                d = min(seg_dist(x + 0.5, y + 0.5, a, b) for a, b in lit) if lit else 99
                put(x, y, shade_at(x, y, d, tone))


def fill(poly, c="."):
    face(poly, [], 0)
    if c != ".":
        xs = [p[0] for p in poly]
        ys = [p[1] for p in poly]
        for y in range(math.floor(min(ys)), math.ceil(max(ys)) + 1):
            for x in range(math.floor(min(xs)), math.ceil(max(xs)) + 1):
                if inside(poly, x + 0.5, y + 0.5):
                    put(x, y, c)


def stamp(x, y, rows):
    for j, row in enumerate(rows):
        for i, c in enumerate(row):
            if c != " ":
                put(x + i, y + j, c)


def axis(tip, direction):
    a = math.atan2(direction[1], direction[0])
    dx, dy = math.cos(a), math.sin(a)

    def at(back, side):
        return (tip[0] - dx * back - dy * side, tip[1] - dy * back + dx * side)
    return at


def quartz(tip, direction, length, half, point, width=1, faces=3, tones=(3, 1, 0),
           ridge="W", blunt=0.0):
    """A quartz point: a six-sided prism seen side on, `faces` (2 or 3) of its body and point
    showing, lit from the left. Its body fades into the dark towards its base, which other
    crystals hide. `width` is the edges' thickness; `ridge` the colour of the edge between the
    lit face and the next; `blunt` flattens the very tip."""
    at = axis(tip, direction)
    if faces == 3:
        sides = (-half, -half * 0.35, half * 0.45, half)
        backs = (point, point * 1.18, point * 1.12, point)
    else:
        sides = (-half, half * 0.2, half)
        backs = (point, point * 1.25, point)
    if blunt:
        top = [at(0, s * blunt) for s in sides]
    else:
        top = [at(0, 0)] * len(sides)
    shoulder = [at(b, s) for s, b in zip(sides, backs)]
    base = [at(length, s) for s in sides]
    out = [top[0], *([top[-1]] if blunt else []), shoulder[-1], base[-1], base[0], shoulder[0]]
    fill(out)
    n = len(sides) - 1
    tone_list = tones[:n] if faces == 3 else (tones[0], tones[2])
    for i in range(n):
        tone = tone_list[i]
        face([top[i], top[i + 1], shoulder[i + 1], shoulder[i]],
             [(top[i], shoulder[i]), (top[i + 1], shoulder[i + 1])] if i == 0 else
             [(top[i], shoulder[i])], min(3, tone + 1))
        face([shoulder[i], shoulder[i + 1], base[i + 1], base[i]],
             [(shoulder[i], shoulder[i + 1])] + ([(shoulder[i], base[i])] if i == 0 else []),
             tone)
    # The point's outline and shoulders in full; the body's edges fade out towards the base.
    tip_line = [shoulder[0], top[0]] + ([top[-1]] if blunt else []) + [shoulder[-1]]
    for a, b in zip(tip_line, tip_line[1:]):
        edge(a, b, width, (0, 1))
    for i in range(n):
        edge(shoulder[i], shoulder[i + 1], width, (0, 1))
    for i in range(1, n):
        edge(top[i], shoulder[i], 1, (1, 0), ridge if i == 1 and ridge else "G")
    for i, s in enumerate(sides):
        a, b = shoulder[i], base[i]
        inner = (1, 0) if i == 0 else (-1, 0)
        for k, x, y in points(*a, *b):
            t = k / max(1, max(abs(round(b[0]) - round(a[0])), abs(round(b[1]) - round(a[1]))))
            if t < 0.45:
                put(x, y, "G" if i in (0, n) else "d")
                if width >= 2 and i in (0, n):
                    put(x + inner[0], y + inner[1], "d")
            elif t < 0.8 and k % 2 == 0:
                put(x, y, "d")


def octahedron(cx, cy, w, h, tilt=0, width=1):
    """A crystal seen point on, as the picture's middle one: four faces about a white cross."""
    top, bottom = (cx + tilt, cy - h), (cx - tilt, cy + h)
    left, right = (cx - w, cy), (cx + w, cy)
    c = (cx, cy)
    fill([top, right, bottom, left])
    face([top, c, left], [(top, left)], 3)
    face([top, right, c], [(top, right)], 2)
    face([left, c, bottom], [(left, bottom)], 1)
    face([c, right, bottom], [(right, bottom)], 0)
    for a, b in [(top, right), (right, bottom), (bottom, left), (left, top)]:
        edge(a, b, width, (1 if a[0] < cx else -1, 0))
    line(top[0], top[1] + 1, *c, "W")
    line(*c, bottom[0], bottom[1] - 1, "G")
    line(left[0] + 1, cy, right[0] - 1, cy, "G")


# ---------------------------------------------------------------------------------------------
# Left: more of the gold crystal cluster.

def seam_pieces(width):
    """What carries on the picture's cut crystals from where their lines leave it: the top edge
    at (0, 72) and (6, 74), the line at 97, the edge from (3, 80), the box crystal's white top
    at line 165 and its gold foot at 190, and the rays."""
    for (x0, y0, x1, y1) in [(0, 141, -40, 121), (0, 154, -46, 131), (0, 190, -30, 182),
                             (0, 200, -44, 196), (-2, 214, -36, 221)]:
        ray(x0, y0, x1, y1)
    apex, lc = (-18, 56), (-38, 70)
    fill([apex, (0, 72), (6, 74), (6, 140), (-38, 140), lc])
    mid = (-20, 82)
    face([apex, lc, mid], [(apex, lc)], 3)
    face([apex, mid, (6, 97), (6, 74), (0, 72)], [(apex, (0, 72)), ((0, 72), (6, 74))], 2)
    face([lc, mid, (-20, 140), (-38, 140)], [(lc, mid), (lc, (-38, 140))], 1)
    face([mid, (6, 97), (6, 140), (-20, 140)], [((-20, 97), (6, 97))], 2)
    edge(apex, (0, 72), width, (0, 1))
    line(0, 72, 6, 74, "G")
    edge(apex, lc, width, (1, 1))
    edge(lc, (-38, 140), width, (1, 0))
    line(apex[0], apex[1] + 1, mid[0], mid[1] - 1, "W")
    line(mid[0], mid[1], -20, 140, "d")
    edge(lc, mid, 1, (0, 1))
    edge(mid, (0, 93), 1, (0, 1))
    line(-20, 97, -1, 97, "G")
    edge((-1, 88), (-16, 118), 1, (-1, 0))
    box = [(-18, 160), (0, 165), (0, 190), (-18, 186)]
    fill(box)
    face(box, [((-18, 160), (0, 165)), ((-18, 160), (-18, 186))], 2)
    line(-18, 160, -1, 165, "W")
    edge((-18, 160), (-18, 186), 2, (1, 0), "W", "G")
    line(-18, 186, -1, 190, "G")


SHARDS = [["  W  ", " mWu ", " mWu ", "mmuuu", " muu "],
          [" W ", "mWu", "mWu", "muu", "mu "],
          ["   Wu", "  mWu", " mWu ", "mmu  ", " u   "],
          ["Wu  ", "mWu ", " mWu", "  mu", "  u "]]


def purple_clump(spots):
    for i, (x, y) in enumerate(spots):
        stamp(x, y, SHARDS[i % 4])


CLUMP = [(-34, 205), (-30, 208), (-38, 210), (-26, 211), (-33, 212), (-42, 213), (-22, 214),
         (-29, 215), (-37, 216), (-46, 217), (-18, 217), (-25, 218)]


FAN = [((-106, 100), (-0.75, -0.66), 70, 13, 15),
       ((-84, 62), (-0.38, -0.92), 90, 15, 18),
       ((-52, 50), (-0.08, -1.0), 80, 14, 17),
       ((-100, 156), (-0.95, -0.3), 55, 11, 13)]


def mirror_seam():
    """Over the cluster mirrored about x 0 (the picture's cells): the picture's crystals are
    kept in front, the mirrored ones pass behind them. The top crystal is closed just left of
    the edge (its left edge from (3, 80) on down, its foot back to (0, 106)); the box crystal
    too (its white top at line 165, its gold foot at 190); the mirrored lines that would meet
    their originals at the edge are cleared, and the picture's own carry on a little."""
    def left_edge(y):
        return 3 - 6 * (y - 80) / 19

    def in_front(x, y):
        return (76 <= y <= 106 and left_edge(min(y, 99)) <= x < 0
                and (x >= -3 or y <= 99) and y <= 99 + 7 * (x + 3) / 3)
    # A crystal's tip rising behind, in the notch between the two peaks.
    tip = [(-5, 65), (0, 71), (0, 76), (-10, 76), (-10, 74)]
    for y in range(65, 77):
        for x in range(-10, 0):
            if inside(tip, x + 0.5, y + 0.5):
                check = (x + y) % 2
                if x < -5:
                    put(x, y, ("G" if check else "d") if y < 72 else ("d" if check else "."))
                else:
                    put(x, y, "d" if check and y < 74 else ".")
    line(-5, 65, -1, 70, "G")
    line(-5, 65, -10, 74, "G")
    line(-5, 66, -5, 70, "W")
    # Behind the seam: clear the mirrored lines.
    for y in range(100, 161):
        for x in range(-12 + max(0, (y - 140) // 4), 0):
            put(x, y, ".")
    for y in range(76, 107):
        for x in range(-8, 0):
            if in_front(x, y):
                put(x, y, "d" if (x + y) % 2 == 0 and y % 3 == 0 else ".")
    edge((0, 87), (-3, 99), 2, (1, 0))
    line(-3, 99, -1, 104, "G")
    line(round(left_edge(97)) + 1, 97, -1, 97, "G")
    # The picture's lines and rays carried on into the cleared space, fading.
    for i, x, y in points(0, 115, -5, 108):
        put(x, y, "W" if i < 3 else "d")
    ray(0, 141, -14, 134)
    ray(0, 154, -14, 148)
    # The box crystal closed: dense checks under its white top, fading lower down.
    for y in range(160, 193):
        for x in range(-16, 0):
            put(x, y, ".")
    face([(-10, 165), (0, 165), (0, 190), (-10, 189)], [((-10, 165), (0, 165)),
                                                        ((-10, 165), (-10, 189))], 3)
    line(-10, 165, -1, 165, "W")
    line(-10, 166, -10, 189, "W")
    line(-11, 166, -11, 188, "G")
    line(-10, 189, -1, 190, "G")


def band_shade(x, y, d, reach):
    """The picture's shading inside a crystal's lit edge, `reach` pixels deep: a gold and dark
    gold check on the edge, then solid dark gold, then dark checks, then sparse specks."""
    k = d * 11 / reach
    check = (x + y) % 2
    if k < 1.5:
        return "G" if check else "d"
    if k < 5:
        return "d" if check or hash01(x, y, 3) < 0.85 else "."
    if k < 8:
        return "d" if check else "."
    if k < 11:
        return "d" if check and (x // 2 + y) % 2 else "."
    return "."


def shaded(poly, lit, reach):
    xs = [p[0] for p in poly]
    ys = [p[1] for p in poly]
    for y in range(math.floor(min(ys)), math.ceil(max(ys)) + 1):
        for x in range(math.floor(min(xs)), math.ceil(max(xs)) + 1):
            if inside(poly, x + 0.5, y + 0.5):
                d = min(seg_dist(x + 0.5, y + 0.5, a, b) for a, b in lit)
                put(x, y, band_shade(x, y, d, reach))


def hex_crystal(apex, shoulders, body, ridge, reach=11):
    """A six-sided crystal drawn as the picture's are: its point's faces meet at `apex`, the
    visible `shoulders` (left to right) where they meet the body, whose edges run along `body`
    (a vector) and fade out. Faces are black, shaded inside their upper edges; the edge to
    shoulder `ridge` is white."""
    bx, by = body
    bottom = [(sx + bx, sy + by) for sx, sy in shoulders]
    silhouette = [apex, shoulders[-1], bottom[-1], bottom[0], shoulders[0]]
    fill(silhouette)
    for i in range(len(shoulders) - 1):
        shaded([apex, shoulders[i], shoulders[i + 1]], [(apex, shoulders[i])],
               reach if i == 0 else reach * 0.6)
        shaded([shoulders[i], shoulders[i + 1], bottom[i + 1], bottom[i]],
               [(shoulders[i], shoulders[i + 1])], reach * (1.0 if i == 0 else 0.7))
    # Edges: the point's outline doubled, the shoulders, the body's fading down.
    for a, b in [(apex, shoulders[0]), (apex, shoulders[-1])]:
        edge(a, b, 2, (0, 1), "G", "G")
    for i, sh in enumerate(shoulders):
        if 0 < i < len(shoulders) - 1:
            line(*apex, *sh, "W" if i == ridge else "G")
        if i + 1 < len(shoulders):
            line(*sh, *shoulders[i + 1], "G")
        for k, x, y in points(*sh, sh[0] + bx, sh[1] + by):
            t = k / max(1, max(abs(bx), abs(by)))
            if t < 0.5 or (t < 0.85 and k % 2 == 0):
                put(x, y, "G" if t < 0.3 and i in (0, len(shoulders) - 1) else "d")


def continued():
    """The picture's cut crystals carried on from where their lines leave it at x 0, and the
    cluster's last crystals beyond them."""
    # Sky down to the crystals, the cluster's black below.
    for y in range(TOP, GROUND_END):
        for x in range(-LEFT, 0):
            put(x, y, "W" if y < 99 else ".")
    # Behind: a crystal leaning out to the left, and a point-on one low down.
    hex_crystal((-54, 70), [(-84, 99), (-62, 106), (-32, 100)], (-14, 44), 1, 9)
    octahedron(-60, 170, 18, 26, 3, 1)
    for (x0, y0, x1, y1) in [(-40, 150, -88, 128), (-44, 190, -88, 200), (-30, 205, -70, 221),
                             (0, 141, -34, 125), (0, 154, -40, 136)]:
        ray(x0, y0, x1, y1)
    # The picture's top-left crystal: its point at (13, 62), its left edge leaving the picture
    # at (0, 73) for its shoulder at (-28, 97), its middle edge leaving at (0, 88) for (-4, 97),
    # the shoulders' line at 97 carried on; its body edges slant down to the right, as the
    # picture's line from (0, 106) does.
    hex_crystal((13, 62), [(-28, 97), (-4, 97), (30, 97), (46, 78)], (14, 30), 2)
    # A crystal behind it whose edges show through below: the picture's white line from
    # (0, 115) and its gold one beside it.
    for i, x, y in points(0, 115, -12, 97):
        put(x, y, "W" if i < 6 else ("G" if i % 2 else "d"))
    # The box crystal: its white top from line 165, its lit face, its gold foot at 190.
    box = [(-12, 165), (0, 165), (0, 190), (-12, 188)]
    fill(box)
    shaded(box, [((-12, 165), (0, 165)), ((-12, 165), (-12, 188))], 9)
    line(-12, 165, -1, 165, "W")
    line(-12, 166, -12, 188, "W")
    line(-13, 167, -13, 187, "G")
    line(-12, 188, -1, 190, "G")


def crystals(design):
    if design == 0:
        return
    if design == 8:
        # The top crystal's corner: its top edge from (0, 72) on to (-3, 75), its doubled left
        # edge down x -3 and -2 to where the middle crystal begins, its face's shading.
        for i, (x, y) in enumerate([(-1, 73), (-2, 74), (-3, 75)]):
            put(x, y, "G")
        for y in range(75, 126):
            put(-3, y, "G" if y < 104 or y % 2 else "d")
            put(-2, y, ("G" if y % 2 else "d") if y < 104 else "d" if y % 3 else ".")
            put(-1, y, "G" if y == 97 else band_shade(-1, y, max(0, y - 74) if y < 97
                                                       else y - 97, 11))
        purple_clump(CLUMP)
        return
    if design == 7:
        continued()
        purple_clump(CLUMP)
        return
    if design == 6:
        mirror_seam()
        return
    for y in range(TOP, GROUND_END):
        for x in range(-LEFT, 0):
            put(x, y, "W" if y < 90 else ".")
    if design == 1:
        # Three-faced quartz points fanning out from the cluster, thin edges.
        for c in FAN:
            quartz(*c, width=1)
        seam_pieces(1)
        octahedron(-52, 172, 20, 27, 3, 1)
    elif design == 2:
        # The same fan, edges doubled (gold, dark gold inside).
        for c in FAN:
            quartz(*c, width=2)
        seam_pieces(2)
        octahedron(-52, 172, 20, 27, 3, 2)
    elif design == 3:
        # Two big points with blunt tips, double edges.
        quartz((-98, 76), (-0.5, -0.87), 95, 20, 22, width=2, blunt=0.3)
        quartz((-58, 50), (-0.12, -0.99), 90, 19, 22, width=2, blunt=0.3)
        seam_pieces(2)
        octahedron(-56, 176, 22, 28, 2, 2)
        quartz((-100, 160), (-0.9, -0.45), 55, 12, 14, width=2)
    elif design == 4:
        # A jumble of smaller points at many angles, thin edges.
        for c in [((-108, 112), (-0.75, -0.66), 50, 10, 12),
                  ((-88, 70), (-0.4, -0.92), 60, 11, 13),
                  ((-62, 56), (-0.12, -0.99), 60, 10, 13),
                  ((-104, 160), (-0.95, -0.3), 45, 9, 11),
                  ((-76, 128), (-0.6, -0.8), 45, 9, 11),
                  ((-50, 112), (-0.2, -0.98), 40, 8, 10)]:
            quartz(*c, width=1, tones=(3, 2, 0))
        seam_pieces(1)
        octahedron(-46, 182, 16, 22, 2, 1)
        octahedron(-86, 198, 13, 18, -2, 1)
    else:
        # Two-faced points (lit and dark, a white ridge between), as the picture's top ones are
        # drawn, double edges.
        for c in FAN:
            quartz(*c, width=2, faces=2)
        seam_pieces(2)
        octahedron(-52, 172, 20, 27, 3, 2)
    purple_clump(CLUMP)


# ---------------------------------------------------------------------------------------------
# Right: the bush made whole, the horizon (the picture's own cells) beyond it, the ground's bands.

def bush(design):
    """The bush made whole: its right half a crown of round clumps of leaves, its peak by the
    picture's edge (line 61 at x 252, as measured), coming down to a trunk that stands on the
    ground, the horizon showing under the crown beside it. `design` picks the trunk."""
    for y in range(TOP, GROUND_END):
        for x in range(256, WIDTH - LEFT):
            put(x, y, "W" if y < HORIZON[0] else ".")
    # The clumps (centre, radius), from the peak round to the crown's underside.
    clumps = [(256, 67, 6), (262, 67, 5), (268, 70, 5), (273, 75, 5), (276, 81, 5),
              (277, 88, 5), (274, 94, 5), (268, 97, 4)]
    core = [(255, 62), (262, 63), (270, 68), (275, 78), (275, 92), (266, 99), (255, 100)]
    trunks = {
        1: [(256, 96), (263, 96), (263, 112), (266, 122), (270, 126), (256, 126)],
        2: [(256, 96), (267, 96), (268, 112), (271, 121), (276, 126), (256, 126)],
        3: [(256, 96), (266, 96), (265, 106), (266, 116), (270, 122), (277, 126), (256, 126)],
    }
    trunk = trunks[design]

    def in_clump(x, y):
        return any((x - cx) ** 2 + (y - cy) ** 2 <= r * r + r for cx, cy, r in clumps)

    def crown(x, y):
        return x < 256 or in_clump(x, y) or inside(core, x + 0.5, y + 0.5)

    def tree(x, y):
        return crown(x, y) or inside(trunk, x + 0.5, y + 0.5)

    xs = range(256, 290)
    ys = range(54, 126)
    for y in ys:
        for x in xs:
            if not tree(x, y):
                continue
            if not crown(x, y):
                # The trunk: black, a dark rim on its right, flecked with gold.
                if not tree(x + 1, y):
                    put(x, y, "G" if y % 3 == 0 else "d")
                elif not tree(x + 2, y) and y % 2:
                    put(x, y, "d")
                else:
                    put(x, y, ".")
                continue
            # The crown's rim, lit from above: how deep under the edge, up or to the right.
            depth = next((k for k in range(8) if not tree(x, y - k - 1) or
                          not tree(x + k + 1, y - k)), 8)
            below = next((k for k in range(4) if not tree(x, y + k + 1)), 4)
            check = (x + y) % 2
            if depth == 0:
                c = "G"
            elif depth == 1:
                c = "G" if check or hash01(x, y, 1) < 0.3 else "d"
            elif depth == 2:
                c = "d" if check else "."
            elif depth <= 5:
                c = "d" if hash01(x, y, 2) < 0.22 - depth * 0.03 else "."
            else:
                c = "."
            if below == 0 and depth > 1:
                c = "d"
            put(x, y, c)
    # Each clump's own curve: a short arc of leaves inside its upper right.
    for cx, cy, r in clumps:
        for k in range(7):
            a = -math.pi / 2 + k * 0.32
            x, y = round(cx + (r - 2.5) * math.cos(a)), round(cy + (r - 2.5) * math.sin(a))
            if crown(x, y - 1) and tree(x, y) and hash01(cx, cy, k) < 0.7:
                put(x, y, "d" if k % 2 else "G")
    # Leaves sticking out of the rim.
    for cx, cy, r in clumps:
        for k in range(3):
            a = -math.pi / 2 + (k - 0.5) * 0.5
            x, y = round(cx + (r + 1) * math.cos(a)), round(cy + (r + 1) * math.sin(a))
            if not tree(x, y) and hash01(cx, k, 5) < 0.5 and y < HORIZON[0]:
                put(x, y, "d")
    # Purple running up the trunk's foot from the ground, as on the picture's side.
    for k in range(10):
        x = 257 + int(hash01(k, 31) * 8)
        y = 112 + int(hash01(k, 32) * 12)
        if tree(x + 2, y):
            put(x, y, "u" if hash01(k, 33) < 0.7 else "m")

    def shows(x, y):
        """Whether the bush is at (x, y)."""
        return 256 <= x < 290 and (tree(x, y) or get(x, y) == "d" and y < HORIZON[0])
    return shows


GRASS_U = [["  m  ", " umu ", "u.u.u"], [" m ", "umu", "u.u"], ["  u ", " mu ", "u.uu"]]
PINK_BUMP = [["  qp  ", " qpWq ", " pqqqp", "qpq.qq"], [" qpp ", "qpqWq", "pqq.q"],
             [" qp ", "qpqq", "q.q."]]
GOLD_GRASS = [["Gd..G ", "GWd.GW", "dd.Gdd"], [" Gd ", "GWd.", "d.dd"], ["G.Gd", "dGdd"]]


def band(y_at, x0, x1, pick, step, seed):
    for x in range(x0, x1, step):
        jx = x + int(hash01(x, seed) * step)
        stamp(jx, round(y_at(jx)) + int(hash01(x, seed + 1) * 3) - 1, pick(hash01(x, seed + 2)))


def ground():
    band(lambda x: 131 - (x - 256) * 0.05, 290, 344, lambda r: GRASS_U[int(r * 3)], 7, 40)
    band(lambda x: 143 - (x - 256) * 0.08, 254, 344, lambda r: PINK_BUMP[int(r * 3)], 6, 50)
    for x in range(256, 344):
        if hash01(x, 55) < 0.45:
            put(x, round(148 - (x - 256) * 0.08) + int(hash01(x, 56) * 3), "q")
    band(lambda x: 157 - (x - 256) * 0.08, 268, 344, lambda r: GOLD_GRASS[int(r * 3)], 4, 60)
    band(lambda x: 160 - (x - 256) * 0.08, 270, 344, lambda r: GOLD_GRASS[int(r * 3)], 5, 65)
    for row in range(5):
        y0 = 172 + row * 10
        x = 252 + int(hash01(row, 80) * 10)
        while x < 346:
            if hash01(x, row, 81) < 0.8 - row * 0.12:
                y = y0 - (x - 256) * 0.16
                for k in range(3 + int(hash01(x, row, 82) * 3)):
                    stamp(x + int(hash01(x, k, 83) * 9) - 2,
                          round(y) + int(hash01(x, k, 84) * 7) - 3,
                          SHARDS[int(hash01(x, row, k, 85) * 4)])
            x += 9 + int(hash01(x, row, 86) * 8)


# ---------------------------------------------------------------------------------------------

def draw(crystal_design, bush_design):
    canvas.clear()
    crystals(crystal_design)
    shows = bush(bush_design)
    ground()
    # The horizon's cells beside the bush and under its side, whatever was drawn there.
    for y in range(*HORIZON):
        for x in range(256, WIDTH - LEFT):
            if not shows(x, y):
                canvas[(x, y)] = None
    for y in range(GROUND_END, HEIGHT):
        for x in list(range(-LEFT, 0)) + list(range(256, WIDTH - LEFT)):
            put(x, y, ".")
    return codes()


def codes():
    """One palette per 8 × 8 cell of the picture's grid (rows from line −2), as on the NES."""
    none = 16
    out = [[none] * WIDTH for _ in range(HEIGHT)]
    for cy in range((TOP + 2) // 8, 31):
        for cx in range(WIDTH // 8):
            pixels = [(x - LEFT, y) for y in range(cy * 8 - 2, cy * 8 + 6) if TOP <= y < HEIGHT
                      for x in range(cx * 8, cx * 8 + 8) if (x - LEFT, y) in canvas]
            votes = {}
            for p in pixels:
                ink = INK.get(canvas[p] or "")
                if ink and ink[1] >= 2:
                    votes[ink[0]] = votes.get(ink[0], 0) + 1
            palette = max(sorted(votes), key=votes.get) if votes else 0
            for (x, y) in pixels:
                c = canvas[(x, y)]
                if c is None:
                    continue
                value = 1 if c == "W" else INK[c][1]
                if value >= 2 and INK[c][0] != palette:
                    value = 2 if c in "Gmp" else 3
                out[y][x + LEFT] = palette * 4 + value if value else 0
    return out


# The PNG: 8-bit indexed; its colours (planet 2's) are only for viewing.
VIEW = [(0, 0, 0), (255, 255, 255), (232, 173, 0), (151, 93, 0),
        (0, 0, 0), (255, 255, 255), (255, 87, 255), (151, 16, 255),
        (0, 0, 0), (255, 255, 255), (255, 101, 167), (215, 22, 87),
        (0, 0, 0), (255, 255, 255), (128, 128, 128), (64, 64, 64), (0, 255, 0)]


def png(rows):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))
    raw = b"".join(b"\0" + bytes(row) for row in rows)
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 3, 0, 0, 0))
            + chunk(b"PLTE", b"".join(bytes(c) for c in VIEW))
            + chunk(b"IDAT", zlib.compress(raw, 9))
            + chunk(b"IEND", b""))


if __name__ == "__main__":
    args = argparse.ArgumentParser()
    args.add_argument("--crystals", type=int, default=8)
    args.add_argument("--bush", type=int, default=2)
    args.add_argument("--out", default="src/game/scene/surface-5-sides.png")
    a = args.parse_args()
    with open(a.out, "wb") as f:
        f.write(png(draw(a.crystals, a.bush)))
