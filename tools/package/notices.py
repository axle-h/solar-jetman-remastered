"""Reads `cargo metadata` JSON on stdin; writes the licence texts of the crates solar-jetman depends on."""
import json, os, sys

meta = json.load(sys.stdin)
pkgs = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
root = next(i for i, p in pkgs.items() if p["name"] == "solar-jetman")

seen, todo = set(), [root]
while todo:
    i = todo.pop()
    if i in seen:
        continue
    seen.add(i)
    for dep in nodes[i]["deps"]:
        if any(k["kind"] in (None, "normal") for k in dep["dep_kinds"]):
            todo.append(dep["pkg"])

print("Third-party software linked into solar-jetman.\n")
for i in sorted(seen, key=lambda i: (pkgs[i]["name"], pkgs[i]["version"])):
    p = pkgs[i]
    if p["name"] == "solar-jetman":
        continue
    print("=" * 72)
    print(f'{p["name"]} {p["version"]}  --  {p.get("license") or "see licence files"}')
    d = os.path.dirname(p["manifest_path"])
    files = sorted(f for f in os.listdir(d) if f.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE")))
    for f in files:
        path = os.path.join(d, f)
        if os.path.isfile(path):
            print(f"\n--- {f}\n")
            print(open(path, encoding="utf-8", errors="replace").read())
    if not files:
        print("\n(no licence file shipped in the crate)")
print()
