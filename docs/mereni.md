# Měření plynulosti

Jak zjistit, kolik požadavků na Open CAD Studio a kolik času stojí úprava
(posun bodu, nový povrch, překreslení pohledu…). Měřit před vydáním a
porovnat s minulou tabulkou.

## Příprava

1. Testovací body (vymyšlená data, mřížka po 2 m):

   ```bash
   python nastroje/body_testovaci.py
   ```

   Vytvoří `ukazka/mereni/body-300.txt`, `body-5000.txt` a `body-20000.txt`
   (do gitu se neukládají).

2. Spustit Open CAD Studio se zapnutým záznamem (PowerShell):

   ```powershell
   $env:TEREN_LADENI = "C:\cesta\zaznam.log"
   & "C:\Program Files\Open CAD Studio\OpenCADStudio.exe" "C:\cesta\vykres.dwg"
   ```

   Terén pak do souboru zapíše každý požadavek („čas Název ms“) a řádky
   `# …` (např. kterou cestou se přepsal blok trasy nebo pohledu).
   Bez proměnné prostředí se nezapisuje nic.

3. Body naimportovat příkazem (Enter jen jednou – druhý Enter import zopakuje):

   ```
   TER_IMPORT_F PYXZD mezera 0 C:\cesta\ukazka\mereni\body-5000.txt
   ```

   Počet bodů ověří `TER_SROVNAT` („bodů N“).

## Měření

Před každou úpravou zapsat značku a pak úpravu provést:

```bash
python nastroje/rozbor_mereni.py znacka znacky.txt "posun bodu"
```

Nakonec rozbor (úprava = požadavky od její značky do další):

```bash
python nastroje/rozbor_mereni.py rozbor znacky.txt C:\cesta\zaznam.log
```

## Co měřit

| Úprava | Jak |
|---|---|
| Import bodů | `TER_IMPORT_F` (5 000 bodů) |
| Nový povrch | `TER_NOVY_STAV` |
| Posun bodu | MOVE popisu bodu o pár metrů |
| Přepočet povrchu | `TER_PREPOCITAT` |
| Překreslení tras a pohledů | `TRA_PREPOCITAT` (2× – podruhé se bloky přepíší na místě) |
| Velký výkres | 20 000 bodů + `TER_NOVY_STAV`: musí skončit povrchem, nebo srozumitelnou chybou o velikosti – nikdy pádem |

## Minulá měření

| Úprava | 17. 11. 2026 | 18. 11. 2026 |
|---|---|---|
| Posun bodu (malý výkres) | 19 požadavků, 7× stažení výkresu | 9 požadavků, zápis 4 |
| Pohled profilu (přepis) | ~12 požadavků | 2 (transakce + SetDirty) |
| 20 000 bodů, nový povrch | chyba 74 MB, pak pád | viz kontrolní seznam |
