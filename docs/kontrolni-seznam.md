# Kontrolní seznam před vydáním

Ruční kontrola v Open CAD Studiu na Windows (10–15 minut). Vychází se
z čistého výkresu a vymyšlených bodů (`python nastroje/body_testovaci.py`).

- [ ] **Načtení pluginu:** záložka Terén na pásu karet, příkazový řádek bez chyb pluginu.
- [ ] **Import bodů:** `TER_IMPORT_F PYXZD mezera 0 …\body-300.txt` – body se značkami a popisy, `TER_SROVNAT` hlásí 324 bodů.
- [ ] **Nový povrch:** trojúhelníky ve 2D celé (všechny hrany), vrstevnice.
- [ ] **Styl povrchu:** změna stylu v Prospektoru překreslí povrch.
- [ ] **Posun bodu:** MOVE popisu i značky – bod se posune, žádný zdvojený popis.
- [ ] **Úprava povrchu:** smazání hrany / trojúhelníku.
- [ ] **Zpět:** po každé úpravě Ctrl+Z vrátí celou úpravu.
- [ ] **Trasa:** nakreslit (Kreslit vrcholy), tah vrcholu úchopem, Zpět.
- [ ] **Pohled profilu:** nový pohled (styl Jednoduchý), niveleta, tah vrcholu nivelety – chytá se terénu (uchopování).
- [ ] **Editor trasy:** tabulka (typ, R, L1, L2), lišta vrcholu, Přidat přechodnice, Posunout přesně, starý výkres → hladiny TER-….
- [ ] **Editor nivelety:** tabulka (R / L / K, sklon, délka), detail vybraného vrcholu, Vložit oblouk klikáním + Enter, Popisy oblouků bez REGEN.
- [ ] **Karta pohledu:** změna stylu, měřítka a převýšení překreslí pohled.
- [ ] **Editor vzhledu profilu:** úprava pásu, Uložit a překreslit.
- [ ] **Režim přepočtu:** přepínač v horní liště Prospektoru; „pozastavit“ → změna trasy nepřepočítá pohled, „Přepočítat teď“ ano.
- [ ] **Uložit, zavřít, znovu otevřít výkres:** body, povrch, trasa i pohled zůstanou a jdou upravovat.
- [ ] **Prospektor:** hodnoty odpovídají výkresu (počet bodů, povrchy, trasy, pohledy).
- [ ] **Velký výkres:** 20 000 bodů + nový povrch – povrch nebo srozumitelná chyba o velikosti, plugin dál reaguje.
- [ ] **Angličtina:** `TRA_JAZYK en` (po restartu) – žádné české texty v nových oknech.
