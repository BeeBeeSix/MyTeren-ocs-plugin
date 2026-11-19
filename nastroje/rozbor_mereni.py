"""Rozbor ladicího záznamu Terénu: počet požadavků a čas na úpravu.

1. Spusťte Open CAD Studio s proměnnou prostředí TEREN_LADENI=<záznam.log>
   (viz docs/mereni.md). Terén do záznamu zapíše každý požadavek na OCS
   („čas Název ms“) a řádky „# …“ (kterou cestou šel přepis bloku apod.).
2. Před každou měřenou úpravou si poznamenejte čas do souboru značek:
       python nastroje/rozbor_mereni.py znacka znacky.txt "posun bodu"
3. Rozbor:
       python nastroje/rozbor_mereni.py rozbor znacky.txt zaznam.log

Úprava = požadavky od její značky do další značky.
"""

import sys
import time
from collections import Counter


def znacka(soubor, nazev):
    with open(soubor, "a", encoding="utf-8") as f:
        f.write(f"{time.time():.3f} {nazev}\n")
    print(f"značka „{nazev}“ zapsána")


def nacti_zaznam(soubor):
    pozadavky, poznamky = [], []
    for radek in open(soubor, encoding="utf-8", errors="replace"):
        c = radek.split()
        if len(c) < 2:
            continue
        try:
            t = float(c[0])
        except ValueError:
            continue
        if c[1] == "#":
            poznamky.append((t, " ".join(c[2:])))
        elif len(c) >= 3:
            try:
                pozadavky.append((t, c[1], int(c[2])))
            except ValueError:
                pass
    return pozadavky, poznamky


def rozbor(znacky_soubor, zaznam_soubor):
    znacky = []
    for radek in open(znacky_soubor, encoding="utf-8"):
        c = radek.strip().split(maxsplit=1)
        if len(c) == 2:
            znacky.append((float(c[0]), c[1]))
    pozadavky, poznamky = nacti_zaznam(zaznam_soubor)
    print(f"{'úprava':32} {'požadavků':>9} {'stažení':>8} {'čekání [s]':>11} {'hotovo za [s]':>14}")
    for i, (t, nazev) in enumerate(znacky):
        t2 = znacky[i + 1][0] if i + 1 < len(znacky) else float("inf")
        cast = [p for p in pozadavky if t <= p[0] < t2]
        if not cast:
            print(f"{nazev:32} {'–':>9}")
            continue
        pocty = Counter(p[1] for p in cast)
        cekani = sum(p[2] for p in cast) / 1000
        print(f"{nazev:32} {len(cast):9} {pocty.get('DocumentSnapshot', 0):8} {cekani:11.1f} {cast[-1][0] - t:14.1f}")
        for k, v in pocty.most_common():
            ms = sum(p[2] for p in cast if p[1] == k)
            print(f"{'':4}{k}: {v}× ({ms} ms)")
        for tp, text in poznamky:
            if t <= tp < t2:
                print(f"{'':4}# {text}")


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "znacka":
        znacka(sys.argv[2], " ".join(sys.argv[3:]))
    elif len(sys.argv) == 4 and sys.argv[1] == "rozbor":
        rozbor(sys.argv[2], sys.argv[3])
    else:
        print(__doc__)
