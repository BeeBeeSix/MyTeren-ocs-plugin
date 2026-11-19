"""Vymyšlené body pro měření a testy (mřížka n × n po 2 m, zvlněný terén).

Použití:
    python nastroje/body_testovaci.py            # tři velikosti do ukazka/mereni/
    python nastroje/body_testovaci.py 50 cesta.txt

Formát řádku: číslo Y X Z popis (oddělené mezerou) – v Terénu import
příkazem `TER_IMPORT_F PYXZD mezera 0 <cesta>`.
Data jsou vymyšlená (žádné skutečné zaměření); stejné n = stejný soubor.
"""

import math
import os
import random
import sys

VELIKOSTI = {"body-300.txt": 18, "body-5000.txt": 71, "body-20000.txt": 142}


def body(n, y0=740000.0, x0=1040000.0, krok=2.0, seed=1):
    rnd = random.Random(seed)
    out = []
    c = 1
    for i in range(n):
        for j in range(n):
            y = y0 + i * krok + rnd.uniform(-0.4, 0.4)
            x = x0 + j * krok + rnd.uniform(-0.4, 0.4)
            # mírné vlny + sklon + údolí uprostřed
            u, v = (y - y0) / 40.0, (x - x0) / 55.0
            z = 302.0 + 1.6 * math.sin(u) + 1.1 * math.cos(v) + 0.004 * (y - y0) - 2.2 * math.exp(-((u - v) ** 2))
            out.append(f"{c} {y:.3f} {x:.3f} {z:.3f} T")
            c += 1
    return out


def zapis(cesta, n):
    os.makedirs(os.path.dirname(os.path.abspath(cesta)), exist_ok=True)
    with open(cesta, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(body(n)) + "\n")
    print(f"{cesta}: {n * n} bodů")


if __name__ == "__main__":
    if len(sys.argv) >= 3:
        zapis(sys.argv[2], int(sys.argv[1]))
    else:
        koren = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "ukazka", "mereni")
        for nazev, n in VELIKOSTI.items():
            zapis(os.path.join(koren, nazev), n)
