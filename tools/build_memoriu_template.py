#!/usr/bin/env python3
"""Construiește șablonul Word al Memoriului tehnic: assets/templates/memoriu-tehnic.docx.

- Textele standard vin din tools/memoriu_texte_standard.txt (secțiuni „### NUME”).
- Datele proiectului sunt variabile `{proiect.nume}`, `{retea.lungime_totala_km}` etc. (vezi
  crates/hidro-core/src/export/document.rs); buclele folosesc sintaxa `{#lista}…{/lista}`.
- Ce nu există în aplicație apare ca „[de completat: …]”, evidențiat cu galben în Word.

Rulare:  python3 tools/build_memoriu_template.py
"""
import re
import sys
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "templates" / "memoriu-tehnic.docx"

# ─── texte standard ──────────────────────────────────────────────────────────

def load_std():
    parts, cur = {}, None
    for line in (ROOT / "tools" / "memoriu_texte_standard.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("### "):
            cur = line[4:].strip()
            parts[cur] = []
        elif cur and line.strip() and not line.startswith("#"):
            parts[cur].append(line.strip())
    return parts

STD = load_std()

# ─── OOXML ───────────────────────────────────────────────────────────────────

def runs(text, bold=False, italic=False, size=None):
    out = ""
    for part in re.split(r"(\[\[.*?\]\])", text):
        if not part:
            continue
        hl = part.startswith("[[")
        t = "[de completat: " + part[2:-2] + "]" if hl else part
        rpr = ""
        if bold:
            rpr += "<w:b/>"
        if italic:
            rpr += "<w:i/>"
        if size:
            rpr += f'<w:sz w:val="{size}"/>'
        if hl:
            rpr += '<w:highlight w:val="yellow"/>'
        out += f'<w:r>{"<w:rPr>" + rpr + "</w:rPr>" if rpr else ""}<w:t xml:space="preserve">{escape(t)}</w:t></w:r>'
    return out


def P(text="", style=None, bold=False, italic=False, align=None, indent=None, hanging=None, size=None, page_break=False, keep=False):
    ppr = ""
    if style:
        ppr += f'<w:pStyle w:val="{style}"/>'
    if keep:
        ppr += "<w:keepNext/>"
    if page_break:
        ppr += "<w:pageBreakBefore/>"
    if indent is not None:
        ppr += f'<w:ind w:left="{indent}"' + (f' w:hanging="{hanging}"' if hanging else "") + "/>"
    if align:
        ppr += f'<w:jc w:val="{align}"/>'
    return f'<w:p>{"<w:pPr>" + ppr + "</w:pPr>" if ppr else ""}{runs(text, bold, italic, size)}</w:p>'


def H1(t, page_break=True):
    return P(t, "Heading1", page_break=page_break)


def H2(t):
    return P(t, "Heading2")


def H3(t):
    return P(t, "Heading3")


def B(t):
    """Element de listă (liniuță cu indent)."""
    return P("•\t" + t, indent=567, hanging=283)


def CB(cond, text):
    """Element de listă afișat doar dacă `cond` e adevărată (paragrafe de buclă, fără liniuță goală)."""
    return P("{#" + cond + "}") + B(text) + P("{/" + cond + "}")


def NB(cond, text):
    """Element de listă afișat doar dacă `cond` e falsă."""
    return P("{^" + cond + "}") + B(text) + P("{/" + cond + "}")


def cell(text, w, bold=False, shade=None, align=None):
    tcpr = f'<w:tcW w:w="{w}" w:type="dxa"/>' + (f'<w:shd w:val="clear" w:color="auto" w:fill="{shade}"/>' if shade else "")
    return f"<w:tc><w:tcPr>{tcpr}</w:tcPr>{P(text, bold=bold, align=align)}</w:tc>"


def table(headers, rows, widths, header_shade="DCE6F2", aligns=None):
    """Tabel cu borduri. Rândurile cu `{#x}` în prima celulă și `{/x}` în ultima se repetă."""
    grid = "".join(f'<w:gridCol w:w="{w}"/>' for w in widths)
    out = f'<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="{sum(widths)}" w:type="dxa"/></w:tblPr><w:tblGrid>{grid}</w:tblGrid>'
    if headers:
        out += '<w:tr><w:trPr><w:tblHeader/></w:trPr>' + "".join(cell(h, w, True, header_shade) for h, w in zip(headers, widths)) + "</w:tr>"
    for r in rows:
        out += "<w:tr>" + "".join(cell(c, w, align=(aligns[i] if aligns else None)) for i, (c, w) in enumerate(zip(r, widths))) + "</w:tr>"
    return out + "</w:tbl>" + P("")


def id_table(rows):
    return table(None, [[k, v] for k, v in rows], [2800, 6200])


# ─── conținut standard → paragrafe ───────────────────────────────────────────

NUM_HEAD = re.compile(r"^\d+(\.\d+)+\.?\s+(.*)$")


def std(name, bullets_all=False):
    lines = STD[name]
    out = []
    for i, l in enumerate(lines):
        nxt = lines[i + 1] if i + 1 < len(lines) else ""
        m = NUM_HEAD.match(l)
        if l == "Întocmit,":
            continue
        if l == "{proiect.proiectant}":
            out.append(P("Întocmit,", align="right"))
            out.append(P("{proiect.proiectant}", bold=True, align="right"))
        elif m:
            out.append(H3(m.group(2).strip()))
        elif l.isupper() and len(l) < 140:
            out.append(H3(l.capitalize()))
        elif bullets_all:
            out.append(B(l.rstrip(";.")+("." if l.endswith(".") else ";") if False else l))
        elif len(l) < 75 and l[0].isupper() and not re.search(r"[.;:,)]$", l) and len(nxt) > len(l):
            out.append(H3(l))
        elif l[0].islower() or l.endswith(";"):
            out.append(B(l))
        else:
            out.append(P(l, align="both"))
    return "".join(out)


# ─── conținutul documentului ─────────────────────────────────────────────────

def build_body():
    b = []
    # Foaia de capăt
    b.append(P("MEMORIU TEHNIC", "Title"))
    b.append(P("{proiect.nume}", "Subtitle"))
    b.append(P("Localitatea {proiect.localitate}, județul {proiect.judet}", align="center"))
    b.append(P(""))
    b.append(P("Beneficiar: {proiect.beneficiar}", align="center"))
    b.append(P("Proiectant: {proiect.proiectant}", align="center"))
    b.append(P("Faza de proiectare: [[P.Th. / D.T.A.C. / altă fază]]", align="center"))
    b.append(P("Număr proiect: [[număr proiect]]", align="center"))
    b.append(P(""))
    b.append(P("Data: {proiect.data}", align="center"))

    # Borderou
    b.append(P("BORDEROU", "Heading1", page_break=True))
    for t in [
        "1. Memoriu general",
        "2. Memoriu tehnic privind sistemul de alimentare cu apă",
        "3. Caiete de sarcini (terasamente, conducte PEHD, cămine din beton)",
        "4. Breviar de calcul",
        "5. Antemăsurătoare (liste de cantități, estimativ)",
        "6. Stabilirea categoriei de importanță a construcției",
        "7. Program de control al calității lucrărilor (Legea nr. 10/1995)",
    ]:
        b.append(P(t))

    # 1. MEMORIU GENERAL
    b.append(H1("1. MEMORIU GENERAL"))
    b.append(H2("1.1 Date generale"))
    b.append(H3("Denumirea obiectivului de investiții"))
    b.append(P("{proiect.nume}"))
    b.append(H3("Amplasamentul"))
    b.append(P("Județul: {proiect.judet}"))
    b.append(P("Localitatea: {proiect.localitate}"))
    b.append(P("Cod poștal: [[cod poștal]]"))
    b.append(P("{#proiect.are_coordonate}Coordonate geografice (aproximative): {proiect.coordonate}{/proiect.are_coordonate}"))
    b.append(H3("Titularul investiției"))
    b.append(id_table([
        ("Denumire", "[[denumire titular]]"), ("Localitatea", "[[localitate]]"), ("Strada", "[[stradă, număr]]"),
        ("Cod poștal", "[[cod poștal]]"), ("Județul", "[[județ]]"), ("Telefon / fax", "[[telefon, fax]]"), ("E-mail", "[[e-mail]]"),
    ]))
    b.append(H3("Beneficiarul investiției"))
    b.append(id_table([
        ("Denumire", "{proiect.beneficiar}"), ("Adresa", "[[localitate, stradă, număr, cod poștal, județ]]"),
        ("Telefon / fax", "[[telefon, fax]]"), ("E-mail", "[[e-mail]]"),
    ]))
    b.append(H3("Elaboratorul proiectului"))
    b.append(id_table([
        ("Proiectant", "{proiect.proiectant}"), ("Adresa", "[[localitate, stradă, număr, cod poștal, județ]]"),
        ("Telefon / fax", "[[telefon, fax]]"), ("E-mail", "[[e-mail]]"),
    ]))
    b.append(H3("Număr proiect și faza de proiectare"))
    b.append(P("Număr proiect: [[număr proiect]]"))
    b.append(P("Faza de proiectare: [[P.Th. / D.T.A.C. / altă fază]]"))

    b.append(H2("1.2 Descrierea generală a lucrărilor"))
    b.append(H3("Amplasamentul"))
    b.append(P("Lucrările se execută în localitatea {proiect.localitate}, județul {proiect.judet}.", align="both"))
    b.append(P("[[descrierea amplasamentului: poziția în comună, vecinătăți, ape de suprafață din zonă]]", align="both"))
    b.append(H3("Topografia"))
    b.append(P("{#retea.are_cote}Localitatea se situează între cotele de nivel topografic {retea.cota_min} – {retea.cota_max} [m], față de nivelul Mării Negre.{/retea.are_cote}", align="both"))
    b.append(P("{^retea.are_cote}[[cotele topografice ale localității; se calculează din cotele tronsoanelor, după ce acestea sunt completate]]{/retea.are_cote}", align="both"))
    b.append(H3("Clima și fenomenele naturale specifice zonei"))
    b.append(P("[[tipul de climă, precipitații, îngheț, fenomene specifice zonei]]", align="both"))
    b.append(H3("Geologia, seismicitatea"))
    b.append(P("[[tipurile de sol, vegetația, zona seismică, stabilitatea amplasamentului]]", align="both"))
    b.append(H3("Prezentarea proiectului pe specialități"))
    b.append(CB("retea.are_tronsoane", "Rețea de distribuție din conducte de polietilenă de înaltă densitate (PEHD), cu lungimea totală de {retea.lungime_totala_km} km;"))
    b.append(B("Rezervor de înmagazinare a apei potabile, pentru compensarea consumului orar, avarii și rezerva intangibilă de incendiu (volum total necesar {rezervor.v_total_rotunjit} [m³]);"))
    b.append(CB("desen.are_camine", "Cămine de vane și de vizitare: {desen.camine_total} buc.;"))
    b.append(CB("desen.are_hidranti", "Hidranți exteriori supraterani: {desen.hidranti} buc.;"))
    b.append(B("[[rețea de aducțiune, stație de injectare cu clor, sistem de contorizare — dacă sunt prevăzute]]"))
    b.append(H3("Devierile și protejările de utilități afectate"))
    b.append(P(STD["DEVIERI"][0], align="both"))
    b.append(H3("Sursele de apă, energie electrică, gaze, telefon și altele asemenea pentru lucrări definitive și provizorii"))
    b.append(P("Sursa de apă existentă / de alimentare: {proiect.sursa_apa}.", align="both"))
    b.append(P("[[utilitățile existente în localitate: surse de apă, energie electrică, gaze naturale, telefonie, internet]]", align="both"))
    b.append(std("UTILITATI").split("<w:p>", 2)[0] if False else "")
    # paragrafele standard despre utilitățile necesare la execuție (fără prima propoziție specifică)
    for l in STD["UTILITATI"][1:]:
        b.append(B(l) if l[0].islower() else P(l, align="both"))
    b.append(H3("Căile de acces permanente, căile de comunicații și altele asemenea"))
    b.append(P("[[drumurile de acces către localitate și în interiorul ei]]", align="both"))
    b.append(H3("Trasarea lucrărilor"))
    for l in STD["TRASARE"]:
        b.append(P(l, align="both"))
    b.append(H3("Antemăsurătoarea"))
    for l in STD["ANTEMASURATOARE"]:
        b.append(P(l, align="both"))
    b.append(H3("Categoria de importanță a obiectivului"))
    b.append(P("Categoria de importanță a construcției: [[categoria stabilită conform HG 766/1997 și STAS 4273/83]]. Stabilirea categoriei de importanță s-a făcut prin acordarea unui punctaj factorilor determinanți (importanță vitală, social-economică și culturală, protecția mediului, durata de utilizare, întreținere), conform procedurii în vigoare. Documentul care atestă categoria de importanță este anexat documentației tehnice.", align="both"))
    b.append(H3("Verificarea proiectului"))
    for l in STD["VERIFICARE"]:
        b.append(P(l, align="both"))
    b.append(P("Întocmit,", align="right"))
    b.append(P("{proiect.proiectant}", bold=True, align="right"))

    # 2. MEMORIU TEHNIC
    b.append(H1("2. MEMORIU TEHNIC PRIVIND SISTEMUL DE ALIMENTARE CU APĂ"))
    b.append(H2("2.1 Legislația de bază"))
    for l in STD["LEGISLATIE"]:
        b.append(B(l))
    b.append(H2("2.2 Situația existentă"))
    b.append(P("Localitatea {proiect.localitate} din județul {proiect.judet} [[nu beneficiază de un sistem centralizat de alimentare cu apă potabilă; descrieți situația existentă]]. Sursa de apă existentă: {proiect.sursa_apa}.", align="both"))
    b.append(H2("2.3 Situația proiectată"))
    b.append(P("Prezenta documentație tehnică prezintă soluția de proiectare a sistemului de alimentare cu apă potabilă destinat localității {proiect.localitate}, județul {proiect.judet}. Soluția propusă urmărește asigurarea necesarului de apă pentru fiecare consumator, în conformitate cu cerințele de calitate, siguranță și continuitate în alimentare. Obiectele sistemului de alimentare cu apă sunt:", align="both"))
    b.append(B("Rețea de distribuție, din conducte de polietilenă de înaltă densitate;"))
    b.append(B("Rezervor pentru stocarea apei potabile;"))
    b.append(B("Hidranți exteriori supraterani și cămine;"))
    b.append(B("[[rețea de aducțiune, stație de injectare cu clor, sistem de contorizare — dacă sunt prevăzute]]"))

    b.append(H3("Cantitățile de apă"))
    b.append(P("În vederea dimensionării corespunzătoare a sistemului, au fost determinate necesarul și cerința de apă, precum și debitele de dimensionare și de verificare, în conformitate cu NP 133/2022.", align="both"))
    b.append(P("Populația de perspectivă, estimată pentru o perioadă de {consum.ani_perspectiva} ani (la o populație actuală de {consum.locuitori} locuitori și o rată de creștere de {consum.rata_crestere} %/an), este de {consum.locuitori_perspectiva} locuitori. Debitele necesarului de apă s-au calculat în funcție de populația de perspectivă, de nevoile gospodărești{#consum.are_consumatori_publici} și de nevoile publice{/consum.are_consumatori_publici}.", align="both"))
    b.append(P("Valorile obținute pentru necesarul de apă sunt:", align="both"))
    b.append(B("necesarul mediu zilnic: Qn zi med = {necesar.qzi_med} [m³/zi];"))
    b.append(B("necesarul maxim zilnic: Qn zi max = {necesar.qzi_max} [m³/zi];"))
    b.append(B("necesarul maxim orar: Qn or max = {necesar.qor_max} [m³/h]."))
    b.append(P("Debitele cerinței de apă s-au determinat în funcție de necesarul de apă și de coeficienții de spor (kp = {coeficienti.kp}, ks = {coeficienti.ks}, kss = {coeficienti.kss}, ksr = {coeficienti.ksr}):", align="both"))
    b.append(B("cerința medie zilnică: Qs zi med = {cerinta.qzi_med} [m³/zi];"))
    b.append(B("cerința maximă zilnică: Qs zi max = {cerinta.qzi_max} [m³/zi];"))
    b.append(B("cerința maximă orară: Qs or max = {cerinta.qor_max} [m³/h] ({cerinta.qor_max_ls} [l/s])."))

    b.append(H3("Apărarea împotriva incendiilor"))
    b.append(P("Având în vedere necesitatea asigurării clădirilor împotriva incendiilor, conform normativelor în vigoare, s-a considerat un număr de incendii simultane n = {incendiu.n_ext}. Hidranții exteriori supraterani au următoarele caracteristici:", align="both"))
    b.append(B("debit hidrant: {incendiu.q_hidrant_ext} [l/s];"))
    b.append(B("presiune minimă de funcționare: {incendiu.p_min_ext} [bar];"))
    b.append(B("timp teoretic de funcționare: {incendiu.t_ext} [ore]."))
    b.append(P("{#desen.are_hidranti}Hidranți exteriori supraterani prevăzuți în desen: {desen.hidranti} buc.{/desen.are_hidranti}", align="both"))
    b.append(P("Debitele de alimentare și de dimensionare, calculate conform NP 133/2022, sunt: QIc = {debite.q_ic} [m³/zi]; Q'Ic = {debite.q_ic_prim} [m³/zi]; QIIc = {debite.q_iic} [m³/h]; QIIv = {debite.q_iiv} [m³/h] ({debite.q_iiv_ls} [l/s]). Rețeaua de distribuție se dimensionează la debitul Qdim = {debite.q_dim_retea} [m³/h].", align="both"))

    b.append(H3("Rezervorul de acumulare a apei"))
    b.append(P("În baza normativelor în vigoare, alimentarea consumatorilor se face prin intermediul unui rezervor, care asigură trei volume de apă: volumul de compensare a consumului orar, volumul pentru avarii pe conducta de aducțiune și volumul pentru incendiu (rezerva intangibilă). Volumele rezultate sunt:", align="both"))
    b.append(B("volumul de compensare: Vcomp = {rezervor.v_comp} [m³];"))
    b.append(B("volumul de avarie: Vav = {rezervor.v_avarie} [m³];"))
    b.append(B("volumul rezervei de incendiu: VRI = {rezervor.v_inc} [m³];"))
    b.append(B("volumul total necesar: Vrez = {rezervor.v_total} [m³] (rotunjit la {rezervor.v_total_rotunjit} [m³])."))
    b.append(P("Caracteristicile rezervorului ales:", align="both"))
    b.append(B("amplasare: {rezervor.amplasare}; material: {rezervor.material}; forma: {rezervor.forma}; număr de cuve: {rezervor.nr_cuve};"))
    b.append(CB("rezervor.are_dimensiuni", "volumul util: {rezervor.capacitate_utila} [m³]; volumul total: {rezervor.capacitate_totala} [m³];"))
    b.append(P("{#rezervor.are_dimensiuni}") + P("{#rezervor.este_cilindric}") + B("diametrul rezervorului: {rezervor.diametru} [mm]; înălțimea totală: {rezervor.inaltime_totala} [mm]; înălțimea rezervorului: {rezervor.inaltime} [mm]; diametrul platformei betonate: {rezervor.platforma_diametru} [mm].") + P("{/rezervor.este_cilindric}") + P("{^rezervor.este_cilindric}") + B("lățimea: {rezervor.latime} [mm]; lungimea: {rezervor.lungime} [mm]; înălțimea totală: {rezervor.inaltime_totala} [mm]; platforma betonată: {rezervor.platforma_lungime} × {rezervor.platforma_latime} [mm].") + P("{/rezervor.este_cilindric}") + P("{/rezervor.are_dimensiuni}"))
    b.append(NB("rezervor.are_dimensiuni", "[[dimensiunile rezervorului ales: volum util, diametru, înălțime, platformă betonată — se completează la pasul „Rezervor”]]"))
    b.append(P("Rezervorul se amplasează la cota terenului de {rezervor.cota_teren} [m]; cota radierului rezultată din calcul este {rezervor.cota_radier} [m], astfel încât să se asigure curgerea gravitațională în rețeaua de distribuție, cu respectarea presiunilor admise. Prin poziția aleasă se respectă distanțele minime de protecție sanitară, conform NP 133/2022.", align="both"))
    b.append(P("Rezervorul se prevede cu set complet de robineți cu plutitor și cu următoarele racorduri: prea plin; alimentare; alimentare rețea de distribuție; pompieri; golire.", align="both"))

    b.append(H3("Rețeaua de distribuție"))
    b.append(P("{#retea.are_tronsoane}Rețeaua de distribuție este alcătuită dintr-un ansamblu de conducte, cămine de vane și hidranți de incendiu exteriori. Conductele sunt din polietilenă de înaltă densitate ([[PEHD PE 100, SDR, PN — de completat]]) și se amplasează subteran, la adâncimea de {retea.adancime_pozare} [m].{/retea.are_tronsoane}", align="both"))
    b.append(P("{#retea.are_tronsoane}Rețeaua are {retea.nr_tronsoane} tronsoane, cu lungimea totală de {retea.lungime_totala_km} km. Gama de diametre și lungimile conductelor sunt:{/retea.are_tronsoane}", align="both"))
    b.append(P("{#retea.diametre}"))
    b.append(B("{lungime_km} km de conductă cu De = {de} [mm] (Di = {di} [mm]);"))
    b.append(P("{/retea.diametre}"))
    b.append(P("{#retea.are_tronsoane}Vitezele reale de curgere sunt cuprinse între {retea.v_min} și {retea.v_max} [m/s], iar pierderea totală de sarcină pe tronsoanele calculate este de {retea.h_total} [m].{/retea.are_tronsoane}", align="both"))
    b.append(P("{#desen.are_camine}Pe rețea sunt prevăzute {desen.camine_total} cămine ({desen.camine_gravitationale} cu golire gravitațională și {desen.camine_aeriene} cu aerisire), pentru manevrarea vanelor, golire și aerisire.{/desen.are_camine}", align="both"))
    b.append(P("{#desen.are_hidranti}Pentru stingerea incendiilor se prevăd hidranți exteriori supraterani: {desen.hidranti} buc.{/desen.are_hidranti}", align="both"))
    b.append(P("[[amplasarea căminelor, dimensiunile lor, subtraversările de drumuri și de căi ferate — dacă este cazul]]", align="both"))

    b.append(H3("Rețeaua de aducțiune"))
    b.append(P("[[descrierea rețelei de aducțiune: traseu, diametre, lungimi, cămine, debit și presiune la punctul de racordare — dacă este prevăzută]]", align="both"))
    b.append(H3("Stația de injectare cu clor"))
    b.append(P("[[descrierea stației de dezinfecție: amplasare, echipamente, principiu de funcționare — dacă este prevăzută]]", align="both"))
    b.append(H3("Sistemul de contorizare"))
    b.append(P("[[amplasarea contoarelor de apă — dacă este prevăzut]]", align="both"))
    b.append(H3("Principiul de funcționare"))
    b.append(P("Rețeaua de distribuție funcționează gravitațional, deoarece rezervorul de stocare se află la o cotă superioară față de consumatori, iar cotele topografice descresc. În lungul traseelor, debitul de apă se poate regla, opri sau porni cu ajutorul căminelor de vane. Rezervorul este prevăzut cu racord de incendiu, pe care se montează o vană de reglare; în caz de incendiu se deschide acest racord, iar incendiul poate fi stins cu ajutorul hidranților exteriori supraterani și al grupurilor de pompieri.", align="both"))

    b.append(H2("2.4 Exploatarea instalațiilor"))
    b.append(std("EXPLOATARE"))

    # 3. CAIETE DE SARCINI
    b.append(H1("3. CAIETE DE SARCINI"))
    b.append(H2("3.1 Caiet de sarcini pentru execuția lucrărilor de terasamente"))
    b.append(std("CAIET_TERASAMENTE"))
    b.append(H2("3.2 Caiet de sarcini pentru conducte din polietilenă de înaltă densitate (PEHD)"))
    b.append(std("CAIET_PEHD"))
    b.append(H2("3.3 Caiet de sarcini pentru cămine din beton"))
    b.append(std("CAIET_CAMINE"))

    # 4. BREVIAR DE CALCUL
    b.append(H1("4. BREVIAR DE CALCUL"))
    b.append(P("Calculele s-au efectuat conform NP 133/2022 și P 118/2-2013. Se prezintă formulele, înlocuirea valorilor și rezultatele.", align="both"))
    b.append(H2("4.1 Populația de perspectivă și necesarul de apă"))
    b.append(P("Ng = N0 · (1 + p/100)^t = {consum.locuitori} · (1 + {consum.rata_crestere}/100)^{consum.ani_perspectiva} = {consum.locuitori_perspectiva} locuitori"))
    b.append(P("Qn zi med gosp = (Ng · qg) / 1000 = ({consum.locuitori_perspectiva} · {consum.q_gosp}) / 1000 = {necesar.qzi_med_gosp} [m³/zi]"))
    b.append(P("{#consum.are_consumatori_publici}Consumatori publici:{/consum.are_consumatori_publici}", bold=True))
    b.append(table(
        ["Nr.", "Tip instituție", "Cantitate", "Nr. consumatori", "Unitate", "q specific", "Ore/zi", "Kor"],
        [["{#consum.consumatori_publici}{nr}", "{tip}", "{cantitate}", "{nr_consumatori}", "{unitate}", "{q_specific}", "{ore_zi}", "{kor}{/consum.consumatori_publici}"]],
        [500, 2300, 1000, 1200, 1100, 1100, 800, 800],
    ))
    b.append(P("Qn zi med = Qn zi med gosp + Qn zi med pub = {necesar.qzi_med_gosp} + {necesar.qzi_med_pub} = {necesar.qzi_med} [m³/zi]"))
    b.append(P("Qn zi max = Qn zi med · Kzi = {necesar.qzi_max_gosp} + {necesar.qzi_max_pub} = {necesar.qzi_max} [m³/zi]   (Kzi gosp = {coeficienti.kzi_gosp}, Kzi pub = {coeficienti.kzi_public})"))
    b.append(P("Qn or max = Qn zi max · Kor / 24 = {necesar.qor_max_gosp} + {necesar.qor_max_pub} = {necesar.qor_max} [m³/h]   (Kor gosp = {coeficienti.kor_gosp}, Kor pub = {coeficienti.kor_public})"))
    b.append(H2("4.2 Cerința de apă"))
    b.append(P("Qs = kp · ks · Qn,   ks = (kss + ksr) − 1 = ({coeficienti.kss} + {coeficienti.ksr}) − 1 = {coeficienti.ks},   kp = {coeficienti.kp}"))
    b.append(P("Qs zi med = {cerinta.qzi_med} [m³/zi];   Qs zi max = {cerinta.qzi_max} [m³/zi];   Qs or max = {cerinta.qor_max} [m³/h] = {cerinta.qor_max_ls} [l/s]"))
    b.append(H2("4.3 Rezerva de incendiu"))
    b.append(P("VRI = a · Qs or max · T + 3,6 · n_inc_ext · Q_ie · T + 3,6 · T_ii · n_inc_int · Q_ii = {incendiu.v_ri} [m³]   (a = {incendiu.a}, Q_ie = {incendiu.q_hidrant_ext} [l/s], T_ie = {incendiu.t_ext} [ore], n_inc_ext = {incendiu.n_ext}, Q_ii = {incendiu.q_hidrant_int} [l/s], T_ii = {incendiu.t_int} [ore], n_inc_int = {incendiu.n_int})"))
    b.append(P("QRI = VRI / TRi · 24 = {incendiu.v_ri} / {incendiu.t_ri} · 24 = {incendiu.q_ri} [m³/zi]"))
    b.append(H2("4.4 Debite de dimensionare"))
    b.append(P("QIc = Qs zi max + kp · ks · QRI = {debite.q_ic} [m³/zi]"))
    b.append(P("Q'Ic = QIc · ksr / ks = {debite.q_ic_prim} [m³/zi]"))
    b.append(P("QIIc = Qs or max + 3,6 · n_inc_int · Q_ii = {debite.q_iic} [m³/h]"))
    b.append(P("QIIv = Qs or max + 3,6 · n_inc_ext · Q_ie = {debite.q_iiv} [m³/h] = {debite.q_iiv_ls} [l/s]"))
    b.append(P("Qdim rețea = max(QIIc, QIIv) = {debite.q_dim_retea} [m³/h]", bold=True))
    b.append(H2("4.5 Rezervorul"))
    b.append(P("hr = i_med · L = {rezervor.i_med} · {rezervor.l_retea} = {rezervor.h_r} [m];   cota radierului: CR = CT + HB + hr = {rezervor.cota_teren} + {rezervor.h_b} + {rezervor.h_r} = {rezervor.cota_radier} [m]"))
    b.append(P("Vcomp = c · Qs zi max = {rezervor.c} · {cerinta.qzi_max} = {rezervor.v_comp} [m³]"))
    b.append(P("Qmin = {rezervor.q_min} [m³/h];   Vav = Qmin · (Tav − Ti) − Qa · Tav = {rezervor.v_avarie} [m³]   (Tav = {rezervor.t_av} [h], Ti = {rezervor.t_i} [h], Qa = {rezervor.q_a} [m³/h])"))
    b.append(P("Vrez = Vcomp + VRI + Vav = {rezervor.v_comp} + {rezervor.v_inc} + {rezervor.v_avarie} = {rezervor.v_total} [m³]", bold=True))
    b.append(H2("4.6 Calculul hidraulic al rețelei de distribuție"))
    b.append(P("{#retea.are_tronsoane}Rugozitatea absolută k = {retea.rugozitate} [m]; vâscozitatea cinematică ν = {retea.viscozitate} [m²/s]; viteza economică v_ec = {retea.viteza_economica} [m/s]. Pierderile de sarcină se calculează cu relația Darcy–Weisbach.{/retea.are_tronsoane}", align="both"))
    b.append(table(
        ["Tr.", "L [m]", "Q_c [l/s]", "D_calc [mm]", "De [mm]", "Di [mm]", "v [m/s]", "Re", "λ", "h_tot [m]"],
        [["{#retea.tronsoane}{nume}", "{lungime}", "{q_c}", "{d_calc}", "{de}", "{di}", "{v}", "{re}", "{lambda}", "{h_tot}{/retea.tronsoane}"]],
        [800, 900, 1000, 1000, 800, 800, 800, 900, 900, 1000],
    ))

    # 5. ANTEMĂSURĂTOARE
    b.append(H1("5. ANTEMĂSURĂTOARE (LISTE DE CANTITĂȚI, ESTIMATIV)"))
    b.append(P("Cantitățile sunt estimate pe baza schemei desenate și a calculului hidraulic. Terasamentele sunt orientative (secțiune dreptunghiulară de șanț) și se verifică la întocmirea devizului.", align="both"))
    b.append(table(
        ["Nr.", "Categoria", "Denumirea", "U.M.", "Cantitatea", "Observații"],
        [["{#cantitati}{nr}", "{categorie}", "{denumire}", "{um}", "{cantitate}", "{observatii}{/cantitati}"]],
        [600, 1700, 3300, 800, 1200, 1500],
    ))

    # 6. CATEGORIA DE IMPORTANȚĂ
    b.append(H1("6. STABILIREA CATEGORIEI DE IMPORTANȚĂ A CONSTRUCȚIEI"))
    b.append(P("Adresa construcției: localitatea {proiect.localitate}, județul {proiect.judet}.", align="both"))
    b.append(P("Obiectivul: {proiect.nume}.", align="both"))
    b.append(P("Categoria de importanță stabilită: [[categoria de importanță]]", align="both"))
    b.append(P("[[tabelul de punctaj al factorilor determinanți (importanță vitală, social-economică și culturală, protecția mediului, durata de utilizare, întreținere) și punctajul total]]", align="both"))

    # 7. PROGRAM DE CONTROL
    b.append(H1("7. PROGRAM DE CONTROL AL CALITĂȚII LUCRĂRILOR, CONFORM LEGII NR. 10/1995"))
    b.append(P("Program de control elaborat de proiectant, în colaborare cu beneficiarul și constructorul, privind controlul calității lucrărilor conform Legii nr. 10/1995. Întocmit la data de {proiect.data_generare}.", align="both"))
    b.append(P("Obiectivul: {proiect.nume}", bold=True))
    b.append(P("Categoria de lucrări: Instalații hidroedilitare"))
    b.append(P("Beneficiar: {proiect.beneficiar}"))
    b.append(P("Executant: [[executant]]"))
    b.append(P("Proiectant: {proiect.proiectant}"))
    b.append(table(
        ["Nr.", "Lucrări ce se controlează, se verifică sau se recepționează calitativ", "Documentul scris care se încheie", "Cine întocmește și semnează", "Nr. și data actului"],
        [
            ["1", "Predare amplasament, stabilirea traseelor și corelarea cu elementele de construcții și cu celelalte instalații", "PV", "B, E, P", ""],
            ["2", "Control dimensional al utilajelor cu montaj", "PV", "B, E", ""],
            ["3", "Control dimensional înainte / în timpul și după montarea instalației", "PV", "B, E", ""],
            ["4", "Probe", "PV", "B, E, P", ""],
            ["5", "Lucrări ascunse", "PVLA", "B, E, P", ""],
            ["6", "Recepția la terminarea lucrărilor", "PVR", "B, E, P, I", ""],
            ["7", "Recepția finală", "PVR", "B, E, P, I", ""],
        ],
        [600, 4000, 1400, 1700, 1300],
    ))
    b.append(P("B – Beneficiar; E – Executant; F – Furnizor; P – Proiectant; I – Inspectoratul de Stat în Construcții. PV – proces-verbal; PVLA – proces-verbal de lucrări ascunse; PVR – proces-verbal de recepție la terminarea lucrărilor.", size=18))
    for l in [
        "Executantul va anunța în scris factorii interesați pentru participarea la verificarea fazei determinante, cu minimum 10 zile înainte de atingerea fazei determinante.",
        "În conformitate cu prevederile legale se interzice trecerea la faza următoare de execuție înainte de recepționarea lucrărilor ajunse în faze determinante.",
        "Coloana cu nr. și data actului încheiat se completează la data încheierii documentului scris.",
        "La recepția obiectivului, un exemplar din prezentul program, cu coloana privind nr. și data actului încheiat completată, se anexează la cartea construcției.",
    ]:
        b.append(B(l))
    b.append(P("Proiectant: {proiect.proiectant}", align="right"))
    return "".join(b)


# ─── pachetul .docx ──────────────────────────────────────────────────────────

NS = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"'

CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
<Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>
</Types>"""

RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"""

DOC_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
</Relationships>"""

STYLES = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles {NS}>
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri" w:eastAsia="Calibri"/><w:sz w:val="22"/><w:lang w:val="ro-RO"/></w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="2400" w:after="240"/><w:jc w:val="center"/></w:pPr><w:rPr><w:b/><w:sz w:val="52"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Subtitle"><w:name w:val="Subtitle"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:after="360"/><w:jc w:val="center"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="240"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="30"/><w:color w:val="1F3A5F"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/><w:color w:val="1F3A5F"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="180" w:after="80"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="23"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Footer"><w:name w:val="footer"/><w:basedOn w:val="Normal"/><w:pPr><w:tabs><w:tab w:val="right" w:pos="9070"/></w:tabs></w:pPr><w:rPr><w:sz w:val="18"/><w:color w:val="555555"/></w:rPr></w:style>
<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:tblPr><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>
<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="19"/></w:rPr><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:left w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:right w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="808080"/></w:tblBorders></w:tblPr></w:style>
</w:styles>"""

FOOTER = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr {NS}><w:p><w:pPr><w:pStyle w:val="Footer"/></w:pPr><w:r><w:t xml:space="preserve">{{proiect.nume}} — Memoriu tehnic</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t xml:space="preserve">Pagina </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:ftr>"""


def document_xml():
    sect = '<w:sectPr><w:footerReference w:type="default" r:id="rId2"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1417" w:right="1417" w:bottom="1417" w:left="1417" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr>'
    return f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<w:document {NS}><w:body>{build_body()}{sect}</w:body></w:document>'


def main():
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CONTENT_TYPES)
        z.writestr("_rels/.rels", RELS)
        z.writestr("word/document.xml", document_xml())
        z.writestr("word/styles.xml", STYLES)
        z.writestr("word/footer1.xml", FOOTER)
        z.writestr("word/_rels/document.xml.rels", DOC_RELS)
    print(f"Scris {OUT} ({OUT.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    sys.exit(main())
