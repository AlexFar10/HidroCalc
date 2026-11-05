# Șabloane pentru exportul documentației

Pasul 9 → panoul **Documentație** generează trei piese scrise:

| Document | Format | Sursă | Șablon |
|---|---|---|---|
| Memoriu tehnic | `.docx` | `src/lib/export/exportMemoriuDOCX.ts` | **obligatoriu** (docxtemplater) |
| Liste de cantități | `.xlsx` | `src/lib/export/exportCantitatiXLSX.ts` | opțional — fără șablon se generează un registru implicit |
| Breviar de calcul | `.pdf` | `src/lib/export/exportBreviarPDF.ts` | nu (generat integral din cod) |

Toate variabilele vin dintr-un singur obiect construit în
`src/lib/export/documentData.ts` (`buildDocumentData`). Exemple gata de pornire:
`docs/sabloane/exemplu-memoriu-tehnic.docx` și `docs/sabloane/exemplu-liste-cantitati.xlsx`.

## Unde se pun șabloanele

1. **Livrate cu aplicația** — copiază fișierele în `public/templates/` cu exact aceste nume:
   - `public/templates/memoriu-tehnic.docx`
   - `public/templates/liste-cantitati.xlsx`

   Sunt folosite automat la „Generează" (și ajung în build-ul desktop).
2. **Ad-hoc** — butonul „Șablon…" deschide un dialog pentru a alege alt fișier.
   În Tauri, fișierul trebuie să fie sub directorul home (scope `fs:scope-home-recursive`).

## Sintaxa în Word (.docx)

- Valoare: `{proiect.nume}`, `{cerinta.qor_max}`
- Buclă: `{#retea.tronsoane}{nume} — De {de} mm, L = {lungime} m{/retea.tronsoane}`
  (într-un tabel: `{#retea.tronsoane}` în prima celulă, `{/retea.tronsoane}` în ultima, pe același rând → un rând per tronson)
- Condiție: `{#consum.are_consumatori_publici}…{/consum.are_consumatori_publici}`; negare: `{^…}…{/…}`
- În interiorul unei bucle, câmpurile elementului se scriu direct (`{nume}`); variabilele globale rămân accesibile (`{proiect.nume}`).
- Numerele sunt formatate românește (virgulă zecimală). Folosește variabilele `are_*` / `este_*` pentru condiții — un număr 0 devine textul „0", care e „adevărat".
- Variabilele necunoscute rămân goale și sunt semnalate printr-un avertisment după export.

> Word sparge uneori un tag în mai multe „run"-uri dacă e editat pe bucăți (corector, formatare parțială).
> Dacă un tag nu e recunoscut, șterge-l și tastează-l din nou dintr-o bucată.

## Sintaxa în Excel (.xlsx)

- Valoare: `{retea.lungime_totala}`. O celulă care conține **doar** tag-ul primește valoarea numerică brută
  (formatul numeric al celulei se aplică); tag-urile din mijlocul unui text sunt înlocuite cu text.
- Rând repetat: orice celulă cu `{lista[].camp}` face ca rândul să fie multiplicat pentru fiecare element:
  `{cantitati[].denumire}`, `{retea.tronsoane[].de}`, `{retea.diametre[].lungime}`.
  Formatarea rândului-șablon se păstrează. O listă goală elimină rândul.
- Formulele care referă intervalul rândului repetat **nu** se extind automat — pentru totaluri folosește
  variabilele scalare (`{retea.lungime_totala}`, `{terasamente.sapatura}`) sau o formulă pe coloană întreagă.

## Variabile disponibile

### `proiect`
`nume`, `localitate`, `judet`, `beneficiar`, `proiectant`, `data`, `sursa_apa`, `data_generare`

### `consum`
`locuitori`, `rata_crestere` [%/an], `ani_perspectiva`, `locuitori_perspectiva`, `q_gosp` [l/om·zi],
`are_consumatori_publici` (bool), `consumatori_publici[]`: `nr`, `tip`, `cantitate`, `nr_consumatori`, `unitate`, `q_specific`, `ore_zi`, `kor`

### `coeficienti`
`kor_gosp`, `kor_public`, `kzi_gosp`, `kzi_public`, `kp`, `kss`, `ksr`, `ks`

### `necesar` (Q_n, m³/zi sau m³/h)
`qzi_med_gosp`, `qzi_med_pub`, `qzi_med`, `qzi_max_gosp`, `qzi_max_pub`, `qzi_max`, `qor_max_gosp`, `qor_max_pub`, `qor_max`

### `cerinta` (Q_s)
`qzi_med`, `qzi_max` [m³/zi], `qor_max` [m³/h], `qor_max_ls` [l/s]

### `incendiu`
Exterior: `q_hidrant_ext` [l/s], `p_min_ext` [bar], `t_ext` [h], `n_ext`, `a`, `nr_hidranti_ext`
Interior: `q_hidrant_int`, `p_min_int`, `t_int`, `n_int`, `nr_hidranti_int`
Rezultate: `v_ri` [m³], `t_ri` [h], `q_ri` [m³/zi]

### `debite`
`q_ic`, `q_ic_prim` [m³/zi], `q_iic`, `q_iiv` [m³/h], `q_iiv_ls` [l/s], `q_dim_retea` [m³/h]

### `rezervor`
Caracteristici: `amplasare`, `material`, `forma`, `pozitie`, `nr_cuve`, `este_cilindric` (bool)
Cote: `cota_teren`, `h_b`, `i_med`, `l_retea`, `h_r`, `cota_radier`
Volume: `c`, `v_comp`, `v_inc`, `v_avarie`, `q_min`, `t_av`, `t_i`, `q_a`, `v_total`, `v_total_rotunjit`
Dimensiuni [mm / m³]: `inaltime_totala`, `inaltime`, `diametru`, `latime`, `lungime`, `capacitate_utila`, `capacitate_totala`

### `retea`
`nr_tronsoane`, `lungime_totala` [m], `adancime_pozare` [m], `rugozitate`, `viscozitate`, `viteza_economica`,
`h_total`, `v_min`, `v_max`, `are_tronsoane` (bool)

`tronsoane[]`: `nr`, `nume`, `grup`, `amonte`, `lungime`, `cota_amonte`, `cota_aval`, `q_lin`, `q_extern`, `q_ie`,
`q_i`, `q_f`, `q_c` [l/s], `d_calc`, `de`, `di` [mm], `v` [m/s], `re`, `regim`, `lambda`, `zeta`,
`h_lin`, `h_loc`, `h_tot` [m], `panta`, `hidrant` (da/nu)

`diametre[]` (conducte grupate pe diametru): `nr`, `de`, `di`, `lungime`, `tronsoane`

### `terasamente` (estimativ — coeficienți în `TRENCH_DEFAULTS`, `src/lib/export/quantities.ts`)
`sapatura`, `nisip`, `umplutura` [m³], `latime_medie`, `adancime_medie` [m]

### `cantitati[]`
`nr`, `categorie`, `denumire`, `um`, `cantitate`, `observatii` — conducte pe diametre, bandă de avertizare,
terasamente, hidranți, cămine (din schema 2D), rezervor, probă de presiune, spălare/dezinfectare.

## Modele 3D

Vederea 3D din Pasul 8 caută modele low-poly în `public/models/`:
`hidrant.gltf`, `rezervor.gltf`, `sursa.gltf`, `camin.gltf` (glTF JSON sau binar; fără compresie Draco —
aplicația e offline). Modelul e scalat automat la înălțimea simbolului, cu baza pe cota terenului.
Dacă fișierul lipsește, se folosește un model procedural.
