# HidroCalc (nativ: Rust + egui)

Aplicație desktop pentru dimensionarea rețelelor ramificate de alimentare cu apă
(NP 133, PM 8/2-2013). Este versiunea nativă a aplicației Tauri + React din
repo-ul `Hidro`: totul e scris în Rust, interfața în [egui](https://github.com/emilk/egui).

## Structură

| Crate | Rol |
|---|---|
| `crates/hidro-core` | Logica de domeniu, fără interfață: modelul `ProjectData` (JSON compatibil cu aplicația React), formulele (pașii 3–8), lanțul de calcul, cotele de teren DEM. |
| `crates/hidrocalc` | Aplicația egui: ecranul de start, pașii, persistența SQLite, traducerile RO/EN. |
| `assets/locales` | Traducerile (aceleași chei ca în aplicația React, plus secțiunea `native`). |
| `assets/fonts` | DejaVu Sans, font de rezervă pentru săgeți și simboluri (licență: `LICENSE_DEJAVU.txt`). |

## Comenzi

```bash
cargo run -p hidrocalc            # pornește aplicația
cargo test                        # teste (calcule, model, DB, interfață)
cargo build --release -p hidrocalc
```

`cargo test -p hidrocalc snapshots` randează fără fereastră ecranul de start și cei
9 pași (temă luminoasă și întunecată) în `target/ui-snapshots/`, pentru verificare vizuală.

## Rețea

Harta folosește dalele `tile.openstreetmap.org` (păstrate în cache-ul aplicației) și căutarea
Nominatim, cu User-Agent propriu, conform politicilor OSM. Cotele de teren vin din dalele publice
Terrarium (SRTM ~30 m). Testele care au nevoie de internet sunt marcate `#[ignore]`:

```bash
cargo test -p hidrocalc map_snapshot -- --ignored   # randare cu hartă reală
cargo test -p hidro-core elevation_known -- --ignored
```

## Date

- Proiectele se salvează în SQLite (`hidro.db`, aceeași schemă ca aplicația Tauri) în
  directorul de date al aplicației (macOS: `~/Library/Application Support/ro.hidro.HidroCalc`).
- La prima pornire, dacă baza e goală, proiectele aplicației Tauri (`ro.hidro.calc/hidro.db`)
  sunt **copiate** aici; baza veche nu se modifică.
- Import/export `.json` compatibil în ambele sensuri cu aplicația React. Câmpurile pe care
  versiunea nativă încă nu le editează (schema rețelei, setările canvas-ului etc.) se păstrează
  neschimbate.

## Stadiul migrării

Toate funcțiile aplicației React/Tauri sunt migrate.

| Funcție | Stare |
|---|---|
| Ecran de start, import/export JSON, salvare automată, RO/EN, temă luminoasă/întunecată | ✅ |
| Pașii 1–7 cu formule și rezultate | ✅ |
| Pasul 8: tabel tronsoane (override-uri), inspector, constante, diametre PEHD, calcul live | ✅ |
| Pasul 8: editor schemă — unelte, snap, zoom/pan, etichete, culori, subsisteme cu porturi, redimensionare, anulare | ✅ |
| Hartă OSM (cache pe disc), ancorare cu Nominatim, opacitate, rotire, blocare pan/zoom/rotire | ✅ |
| Cote teren automate din DEM și din puncte topo (CSV/GPX/KML, Stereo 70) | ✅ |
| Vedere 3D: teren DEM, conducte la adâncimea de pozare, noduri, etichete | ✅ (fără modele .gltf) |
| Pasul 9: indicatori, grafice, tabel rezultate | ✅ |
| Exporturi: PDF raport, breviar PDF, memoriu .docx din șablon, cantități .xlsx (implicit sau din șablon), DXF, PNG | ✅ |

Diferențe față de versiunea React: vederea 3D e desenată cu egui (fără modele .gltf, cu modele procedurale);
PDF-urile au diacritice (fonturi încorporate); captura PNG are rezoluția ecranului.
