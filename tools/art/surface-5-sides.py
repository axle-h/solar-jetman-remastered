#!/usr/bin/env python3
"""Draws src/game/scene/surface-5-sides.png, surface picture 5's sides in a 16:9 frame
(docs/widescreen.md), from shapes alone: crystals, lines, a hill, bushes, bands of ground.
Deterministic, standard library only. Coordinates are the picture's (x 0-255, lines 0-239); the
art covers x -88 to 343, and on the left also column 0 (x 0-7), which the original masks. The
points where a shape meets the picture's edge were measured on the picture, column 0's hidden
pixels included.

Run from the repository root:
    python3 tools/art/surface-5-sides.py [--bush N] [--out FILE]
N picks the bush's trunk (1-3); the default is the chosen one.
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

# The art may also cover column 0 (x 0-7), masked on the original, where the picture's hidden
# pixels would carry its crystals on beyond what the original shows.
COLUMN_0_END = 8

canvas = {}
# The pixels inside the shapes filled so far (the crystals): the ground never shows there.
solid = set()


def put(x, y, c):
    if -LEFT <= x < WIDTH - LEFT and TOP <= y < HEIGHT and not COLUMN_0_END <= x < 256:
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


def fill(poly, c="."):
    """Fills `poly` with `c` (black by default), its pixels then solid: the ground stays out."""
    xs = [p[0] for p in poly]
    ys = [p[1] for p in poly]
    for y in range(math.floor(min(ys)), math.ceil(max(ys)) + 1):
        for x in range(math.floor(min(xs)), math.ceil(max(xs)) + 1):
            if inside(poly, x + 0.5, y + 0.5):
                put(x, y, c)
                solid.add((x, y))


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


# ---------------------------------------------------------------------------------------------
# Left: more of the gold crystal cluster.

SHARDS = [["  W  ", " mWu ", " mWu ", "mmuuu", " muu "],
          [" W ", "mWu", "mWu", "muu", "mu "],
          ["   Wu", "  mWu", " mWu ", "mmu  ", " u   "],
          ["Wu  ", "mWu ", " mWu", "  mu", "  u "]]


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


def skyline(knots, x0, x1, seed, lump=1.5):
    """A silhouette's top line from `knots` (x, y), with lumps of leaves along it: y by x."""
    tops = {}
    for x in range(x0, x1):
        for (ax, ay), (bx, by) in zip(knots, knots[1:]):
            if ax <= x <= bx:
                y = ay + (by - ay) * (x - ax) / (bx - ax or 1)
                break
        else:
            y = knots[-1][1]
        # Round lumps a few pixels wide, as the picture's hill and bush have.
        k = x // 5
        y -= lump * math.sin(math.pi * (x - k * 5) / 5) * (0.5 + hash01(k, seed))
        tops[x] = round(y)
    return tops


def rimmed(tops, bottom, rim, seed):
    """Black under `tops` down to line `bottom`, its upper edge lit as the picture's hill (rim
    "purple") or bush (rim "gold") are: bright on the edge, fading in a few pixels."""
    for x, top in tops.items():
        for y in range(top, bottom):
            # How deep under the edge, up or along the slope.
            depth = min([y - top] + [abs(x - nx) - 1 for nx in range(x - 4, x + 5)
                                     if nx in tops and tops[nx] > y])
            h = hash01(x, y, seed)
            check = (x + y) % 2
            if rim == "purple":
                c = ("m" if h < 0.7 else "u") if depth == 0 else \
                    ("u" if h < 0.6 else "m") if depth == 1 else \
                    ("u" if h < 0.7 else ".") if depth == 2 else \
                    ("u" if h < 0.25 else ".") if depth == 3 else "."
            else:
                c = ("G" if check or h < 0.4 else "d") if depth == 0 else \
                    ("d" if check or h < 0.3 else "G") if depth == 1 else \
                    ("d" if check else ".") if depth == 2 else \
                    ("d" if check and h < 0.5 else ".") if depth <= 4 else "."
            put(x, y, c)
        # A pixel of leaves sticking up here and there.
        if hash01(x, seed, 7) < 0.2 and tops.get(x - 1, 0) >= top and tops.get(x + 1, 0) >= top:
            put(x, top - 1, ("m" if rim == "purple" else "G"))


def specks(x0, x1, y0, y1, seed, density, c="u"):
    """Purple specks in the dark at a silhouette's foot."""
    for y in range(y0, y1):
        for x in range(x0, x1):
            if hash01(x, y, seed) < density * (1 - (y - y0) / (y1 - y0)):
                put(x, y, c)


def left_horizon():
    """The skyline left of the cluster, in the order the picture's runs right of it: by the
    cluster a hill rimmed in purple, its far foot at the cell edge x -48; then a bush rimmed in
    gold (x -72 to -50) sloping down into a low hedge that hides the horizon to the frame's
    edge."""
    hill = skyline([(-48, 104), (-44, 95), (-38, 90), (-31, 88), (-25, 82), (-18, 79),
                    (-11, 82), (-6, 85), (0, 84), (8, 86)], -48, COLUMN_0_END, 20, 2.5)
    rimmed(hill, 126, "purple", 21)
    specks(-48, -32, 112, 124, 22, 0.035)
    bush = skyline([(-89, 101), (-83, 100), (-77, 99), (-72, 96), (-67, 93), (-58, 93),
                    (-53, 95), (-50, 101), (-49, 104)], -88, -48, 30, 2)
    rimmed(bush, 126, "gold", 31)
    # Under the bush and hedge, purple specks in the dark, as under the picture's bush.
    specks(-88, -48, 110, 124, 32, 0.07)


def outline_x(y):
    """The top crystal's left outline: the picture's ridge through (5, 76) and (0, 87), on
    down to the crystal's left point at (-6, 97)."""
    return 7 - 13 * (y - 72) / 25


def top_crystal_close():
    """The top crystal closed at its left point (-6, 97), where its outline meets the
    shoulders' gold line at 97. Column 0's hidden pixels outside the outline (more of the
    crystal than the original shows) give way to the sky and the hill. Below the point, the
    body's edge runs on down to the picture's gold line leaving at (0, 106)."""
    point = (-6, 97)
    # Outside the outline in column 0: the sky, the hill as drawn, or the hill's dark.
    for y in range(60, 98):
        for x in range(-12, COLUMN_0_END):
            if x >= outline_x(y):
                if x < 0:
                    put(x, y, ".")
                else:
                    canvas.pop((x, y), None)
                continue
            if x >= 0 and get(x, y) is None:
                put(x, y, "W" if y < 84 else ".")
    # Column 0 below the point is the picture's own.
    for y in range(98, 126):
        for x in range(COLUMN_0_END):
            canvas.pop((x, y), None)
    # Inside the outline: the face's shading, then the body's small face under the point.
    face_poly = [(0, 87), point, (0, 97)]
    fill(face_poly)
    shaded(face_poly, [((0, 87), point)], 7)
    below = [point, (0, 97), (0, 106)]
    fill(below)
    shaded(below, [(point, (0, 97))], 7)
    edge((7, 72), point, 2, (1, 0), "G", "G")
    line(point[0] + 1, 97, -1, 97, "G")
    line(point[0], 98, -1, 105, "G")


def box_shade(x, y, d, reach):
    """The picture's box crystal's shading inside a lit edge, `reach` pixels deep: a thin
    strip of gold by the edge, a gold and dark gold check, solid dark gold, dark checks,
    sparse specks."""
    k = d * 11 / reach
    check = (x + y) % 2
    if k < 0.9:
        return "G"
    if k < 2.2:
        return "G" if check else "d"
    if k < 5:
        return "d"
    if k < 8:
        return "d" if check else "."
    if k < 11:
        return "d" if check and (x // 2 + y) % 2 else "."
    return "."


def seg_dist_t(px, py, a, b):
    """The distance from (px, py) to segment a-b, and how far along it (0-1) the nearest point is."""
    (ax, ay), (bx, by) = a, b
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy or 1)))
    return math.hypot(px - ax - t * dx, py - ay - t * dy), t


# How deep the new crystals' shading reaches, against the depths their faces are given: the
# picture's crystals (the one at column 9, rows 9-16, above all) are shaded only 2-3 pixels in
# from an edge, black inside.
SHADE_DEPTH = 0.75


def box_shaded(poly, lit):
    """Shades `poly` from its edges, each (a, b, reach), as on the picture's box crystals: an
    edge's shading reaches deepest at its ends and least at its middle (to `reach` × 0.55), and
    the edges' distances blend (a harmonic mean), so that the shading swells into the corners
    and the dark inside rounds off."""
    xs = [p[0] for p in poly]
    ys = [p[1] for p in poly]
    for y in range(math.floor(min(ys)), math.ceil(max(ys)) + 1):
        for x in range(math.floor(min(xs)), math.ceil(max(xs)) + 1):
            if inside(poly, x + 0.5, y + 0.5):
                total = 0.0
                for a, b, reach in lit:
                    d, t = seg_dist_t(x + 0.5, y + 0.5, a, b)
                    r = SHADE_DEPTH * reach * (0.55 + 0.45 * abs(2 * t - 1) ** 2)
                    total += 1 / (max(0.3, d) * 11 / r)
                k = 1 / total + (hash01(x, y, 9) - 0.5) * 0.6
                put(x, y, box_shade(x, y, max(0.0, k), 11))


def box_crystal():
    """A crystal seen end on below the top one, shaded as the picture's box crystals are: its
    front face (x -31 to -6, lines 105-136) with white top and left edges and a white
    diagonal, gold right and bottom edges; its side face receding to the picture's edge
    between the picture's white line leaving at (0, 115) and its gold one at (0, 141), whose
    shading column 0 already shows; its bottom face below. A ray from under it runs on to
    the picture's at (0, 154)."""
    # The mirrored cells' lines under it are cleared.
    for y in range(106, 163):
        for x in range(-16, 0):
            put(x, y, ".")
    tl, tr, br, bl = (-31, 105), (-6, 105), (-6, 136), (-31, 136)
    front = [tl, tr, br, bl]
    side = [tr, (0, 115), (0, 141), br]
    bottom = [bl, br, (0, 141), (-24, 142)]
    for poly in (front, side, bottom):
        fill(poly)
    # The front face: lit from its top and left edges, and the diagonal lights both its halves
    # a little, as the picture's box's does.
    upper = [tl, tr, br]
    lower = [tl, br, bl]
    # As deep as the picture's box crystal at (13, 19) is shaded.
    box_shaded(upper, [(tl, tr, 7), (tl, br, 4), (tr, br, 2.5)])
    box_shaded(lower, [(tl, bl, 7), (tl, br, 4), (bl, br, 2.5)])
    box_shaded(side, [(tr, (0, 115), 6), (tr, br, 3)])
    box_shaded(bottom, [(bl, br, 4)])
    line(*tl, tr[0], tr[1], "W")
    line(*tl, *bl, "W")
    # The diagonal: white by the corners, toning down to gold in the middle, as the picture's.
    n = max(br[0] - tl[0], br[1] - tl[1]) - 2
    for i, x, y in points(tl[0] + 1, tl[1] + 1, br[0] - 1, br[1] - 1):
        middle = 1 - abs(2 * i / n - 1)
        put(x, y, "W" if middle < 0.4 else "G")
    line(*tr, *br, "G")
    line(tr[0] + 1, tr[1] + 2, br[0] + 1, br[1] - 1, "d")
    line(*bl, *br, "G")
    # The side face's edges: the white one on to (0, 115), the gold one on to (0, 141); the
    # bottom face's lower edge fading out to the left.
    for _, x, y in points(*tr, -1, 114):
        put(x, y, "W")
    for _, x, y in points(*br, -1, 140):
        put(x, y, "G")
    for i, x, y in points(-1, 141, -24, 142):
        if i < 12 or i % 2 == 0:
            put(x, y, "G" if i < 8 else "d")
    ray(-1, 154, -18, 145)


def edge_crystal():
    """The crystal the edge cuts low down, whose point, white ridge and right faces the
    picture and column 0 show, made a box on the left: its white top edge rising from a corner
    at (-17, 169) to the point by x 0 (line 165); its left edge down to (-16, 191), white then
    gold; its bottom edge the picture's line leaving at (0, 190); its diagonal the picture's
    gold line leaving at (0, 183), white by the corner. Shaded as the box above."""
    for y in range(156, 196):
        for x in range(-17, 0):
            put(x, y, ".")
    tl, top, diag, foot, bl = (-17, 169), (0, 165), (0, 183), (0, 190), (-16, 191)
    upper = [tl, top, diag]
    lower = [tl, diag, foot, bl]
    for poly in (upper, lower):
        fill(poly)
    box_shaded(upper, [(tl, top, 7), (tl, diag, 3.5)])
    box_shaded(lower, [(tl, bl, 5), (tl, diag, 3.5)])
    line(*tl, -1, 165, "W")
    for i, x, y in points(*tl, *bl):
        put(x, y, "W" if i < 12 else "G")
    for i, x, y in points(tl[0] + 1, tl[1] + 1, -1, 182):
        put(x, y, "W" if i < 7 else "G")
    line(*bl, -1, 190, "G")


def fading(a, b, bright=0.35, last=0.8):
    """A body's edge from a, gold then dark gold, dotted towards b, where it fades out."""
    n = max(1, max(abs(round(b[0]) - round(a[0])), abs(round(b[1]) - round(a[1]))))
    for i, x, y in points(*a, *b):
        t = i / n
        if t < bright:
            put(x, y, "G")
        elif t < last and (t < 0.55 or i % 2 == 0):
            put(x, y, "d")


def tall_crystal():
    """A tall six-sided crystal standing in front of the hill's dark, its point up: the tip at
    (-46, 146), shoulders at (-56, 169), (-40, 174) and (-26, 160), a white ridge from the tip
    to the middle one, its body's edges fading out below."""
    a, l, m, r = (-46, 146), (-56, 169), (-40, 174), (-26, 160)
    lb, mb, rb = (-45, 210), (-35, 205), (-23, 196)
    left_point, right_point = [a, l, m], [a, m, r]
    left_body, right_body = [l, m, mb, lb], [m, r, rb, mb]
    for poly in (left_body, right_body, left_point, right_point):
        fill(poly)
    box_shaded(left_body, [(l, m, 6), (l, lb, 4), (m, mb, 2)])
    box_shaded(right_body, [(m, r, 5), (m, mb, 3), (r, rb, 2)])
    # The point's faces mostly black inside, shaded only along their edges, as the picture's.
    box_shaded(left_point, [(a, l, 4), (a, m, 2), (l, m, 1.2)])
    box_shaded(right_point, [(a, r, 3.5), (a, m, 1.8), (m, r, 1.2)])
    # The body fades into the dark towards its foot.
    for y in range(185, 212):
        for x in range(-58, -20):
            if get(x, y) not in (None, ".") and hash01(x, y, 12) < (y - 185) / 26:
                put(x, y, ".")
    fading(l, lb)
    fading(m, mb)
    fading(r, rb)
    # Gold outlines, as the picture's crystals have; only the ridge is white.
    edge(a, l, 2, (1, 0), "G", "d")
    edge(a, r, 2, (0, 1), "G", "d")
    for i, x, y in points(a[0], a[1] + 1, *m):
        put(x, y, "W" if i < 6 or i > 20 else "G")
    line(*l, *m, "G")
    line(m[0] + 1, m[1] - 1, *r, "G")


def pyramid_crystal():
    """A small crystal seen point on, under the edge crystal: its tip at (-19, 191), its
    corners at (-32, 207), (-3, 202) and (-14, 213), a white ridge from the tip to the lowest."""
    p, lc, rc, c = (-19, 191), (-32, 207), (-3, 202), (-14, 213)
    left, right = [p, lc, c], [p, c, rc]
    for poly in (left, right):
        fill(poly)
    box_shaded(left, [(p, lc, 4), (p, c, 2), (lc, c, 1.2)])
    box_shaded(right, [(p, rc, 3.5), (p, c, 1.8), (c, rc, 1.2)])
    line(*p, *lc, "G")
    line(*p, *rc, "G")
    for i, x, y in points(p[0], p[1] + 1, *c):
        put(x, y, "W" if i < 6 or i > 17 else "G")
    line(*lc, *c, "G")
    line(*c, *rc, "G")
    fading(lc, (-30, 221), 0.4, 1.0)
    fading(c, (-12, 221), 0.4, 1.0)


def leaning_crystal(tip, direction, point, half, body):
    """A six-sided crystal drawn as the tall one is, its point at `tip` facing `direction`
    (the point `point` pixels long, `half` pixels across, its body `body` pixels long, fading
    into the ground): two faces of its point about a white ridge, two of its body."""
    at = axis(tip, direction)
    a = at(0, 0)
    l, m, r = at(point, -half), at(point * 1.2, half * 0.15), at(point, half)
    # Its right edges run on further down, the body leaning away from the light.
    lb, mb, rb = at(point + body, -half * 0.9), at(point * 1.2 + body * 1.25, half * 0.15), \
        at(point + body * 1.45, half * 0.9)
    left_point, right_point = [a, l, m], [a, m, r]
    left_body, right_body = [l, m, mb, lb], [m, r, rb, mb]
    for poly in (left_body, right_body, left_point, right_point):
        fill(poly)
    # Shading as deep as the tall crystal's, for its size.
    k = half / 12
    box_shaded(left_body, [(l, m, 6 * k), (l, lb, 4 * k), (m, mb, 2)])
    box_shaded(right_body, [(m, r, 5 * k), (m, mb, 3 * k), (r, rb, 2)])
    # The point's faces mostly black inside, shaded only along their edges, as the picture's.
    box_shaded(left_point, [(a, l, 4 * k), (a, m, 2 * k), (l, m, 1.2)])
    box_shaded(right_point, [(a, r, 3.5 * k), (a, m, 1.8 * k), (m, r, 1.2)])
    # The body dissolves gently into the dark towards its foot: fewer and fewer of its
    # shading's pixels from a fifth of the way down, and only dark gold below the middle.
    for poly in (left_body, right_body):
        xs = [p[0] for p in poly]
        ys = [p[1] for p in poly]
        for y in range(math.floor(min(ys)), math.ceil(max(ys)) + 1):
            for x in range(math.floor(min(xs)), math.ceil(max(xs)) + 1):
                if not inside(poly, x + 0.5, y + 0.5) or get(x, y) in (None, "."):
                    continue
                down = ((x - l[0]) * -direction[0] + (y - l[1]) * -direction[1]) / (body * 1.3)
                if hash01(x, y, 13) < max(0.0, (down - 0.2) / 0.75):
                    put(x, y, ".")
                elif down > 0.4 and get(x, y) == "G":
                    put(x, y, "d")
    fading(l, lb, 0.25, 0.9)
    fading(m, mb, 0.2, 0.95)
    fading(r, rb, 0.2, 0.95)
    edge(a, l, 2, (1, 0), "G", "d")
    for i, x, y in points(*a, *r):
        put(x, y, "G")
    n = point * 1.2
    for i, x, y in points(*a, *m):
        put(x, y, "W" if i < n * 0.3 or i > n * 0.75 else "G")
    line(*l, *m, "G")
    line(*m, *r, "G")


def left_ground():
    """The ground's bands carried on left of the cluster, as on the right but rising to the
    left: purple grass, pink crystals, gold grass, clumps of purple crystals. Behind the
    crystals (never inside one), and in a cell the crystals made gold only the gold grass, so
    every cell keeps one palette."""
    drawn = dict(canvas)
    canvas.clear()
    band(lambda x: 131 + x * 0.05, -88, -40, lambda r: GRASS_U[int(r * 3)], 7, 140)
    band(lambda x: 143 + x * 0.08, -88, -30, lambda r: PINK_BUMP[int(r * 3)], 6, 150)
    for x in range(-88, -30):
        if hash01(x, 155) < 0.45:
            put(x, round(148 + x * 0.08) + int(hash01(x, 156) * 3), "q")
    band(lambda x: 157 + x * 0.08, -88, -6, lambda r: GOLD_GRASS[int(r * 3)], 4, 160)
    band(lambda x: 160 + x * 0.08, -88, -8, lambda r: GOLD_GRASS[int(r * 3)], 5, 165)
    for row in range(5):
        y0 = 172 + row * 10
        x = -92 + int(hash01(row, 180) * 10)
        while x < -4:
            if hash01(x, row, 181) < 0.8 - row * 0.12:
                y = y0 + x * 0.16
                for k in range(3 + int(hash01(x, row, 182) * 3)):
                    stamp(x + int(hash01(x, k, 183) * 9) - 2,
                          round(y) + int(hash01(x, k, 184) * 7) - 3,
                          SHARDS[int(hash01(x, row, k, 185) * 4)])
            x += 9 + int(hash01(x, row, 186) * 8)
    ground = dict(canvas)
    canvas.clear()
    canvas.update(drawn)

    def cell(x, y):
        return (x // 8, (y + 2) // 8)
    gold = {cell(x, y) for (x, y), c in drawn.items() if c in ("G", "d")}
    for (x, y), c in ground.items():
        if (x, y) in solid or not 104 <= y < GROUND_END or x >= 0 or c == ".":
            continue
        if get(x, y) not in (None, "."):
            continue
        if cell(x, y) in gold and c not in ("G", "d"):
            continue
        canvas[(x, y)] = c


def left_side():
    """The left side, back to front: the skyline, the cluster's crystals carried on and
    closed, the ground's bands behind them."""
    left_horizon()
    top_crystal_close()
    box_crystal()
    # A big crystal behind the tall and small ones, as big as the top one, pointing between
    # their directions.
    leaning_crystal((-66, 114), (-0.31, -0.95), 24, 22, 78)
    tall_crystal()
    edge_crystal()
    pyramid_crystal()
    # A small crystal pointing up and to the left, by the tall one's foot.
    leaning_crystal((-77, 185), (-0.5, -0.87), 14, 11.5, 24)
    left_ground()


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

def draw(bush_design):
    canvas.clear()
    solid.clear()
    left_side()
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
    args.add_argument("--bush", type=int, default=2)
    args.add_argument("--out", default="src/game/scene/surface-5-sides.png")
    a = args.parse_args()
    with open(a.out, "wb") as f:
        f.write(png(draw(a.bush)))
