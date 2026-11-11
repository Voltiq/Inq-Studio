# Inq Studio — Anleitung

Inq Studio ist ein PDF-Betrachter und -Editor für Windows. Diese Anleitung geht durch, was er kann, in der
Reihenfolge, in der man es meist braucht. *An English version is in [GUIDE.md](GUIDE.md).*

## Das Fenster

- **Obere Leiste**: links Öffnen, Speichern, Rückgängig und Wiederholen; in der Mitte Name und Seite des
  Dokuments und seine Kennzeichen (*signiert*, Art des Kennwortschutzes, PDF/A-Angabe — ein Klick öffnet den
  passenden Dialog); rechts Design, **Dokumentwerkzeuge** (der Schraubenschlüssel), Einstellungen und
  Inspektor.
- **Seitenleiste** (`Strg+B`): Seitenminiaturen, Lesezeichen, Schriften, Kommentare, Verlauf.
- **Werkzeugleiste** über der Seite: die unten beschriebenen Werkzeuge, Blättern, Zoom, Suche.
- **Inspektor** (`Strg+I`): was ausgewählt ist, die Optionen des aktiven Werkzeugs und die
  Dokumenteigenschaften.
- Mehrere Dokumente öffnen sich als Tabs. Ein im Explorer geöffnetes PDF kommt in das bereits offene Fenster.

Jede Änderung lässt sich rückgängig machen (`Strg+Z`), bis das Dokument geschlossen wird; der Tab **Verlauf**
listet die Schritte und springt zu jedem zurück. In die Datei wird erst beim Speichern geschrieben (`Strg+S`).

## Lesen

Scrollen oder `Bild auf` / `Bild ab`. `Strg`+Mausrad zoomt; `Strg+0` passt die Breite an. `Strg+F` sucht Text.
Mit dem Werkzeug **Auswählen** (`V`) wird Text markiert und kopiert; die **Hand** (`H`) verschiebt die Seite.

## Text bearbeiten

**Text und Bilder bearbeiten** (`E`) wählen. Jede Zeile erhält einen Rahmen.

- **Doppelklick** auf eine Zeile schreibt sie neu. Gehört die Zeile zu einem Absatz, öffnet sich der ganze
  Absatz und wird beim Bestätigen mit `Strg+Eingabe` neu auf seine Spalte umbrochen.
- **Ziehen** verschiebt eine Zeile; sie rastet an Kanten und Grundlinien anderer Zeilen ein (`Alt` verschiebt
  frei). Die Pfeiltasten verschieben fein. `Entf` löscht. `Umschalt`-Klick oder ein aufgezogener Rahmen wählt
  mehrere Zeilen.
- Der Inspektor ändert Größe, Farbe und Schrift.

Was Sie tippen, wird **in der Schrift geschrieben, die das Dokument bereits verwendet**. Fehlt dieser Schrift
ein Zeichen, nimmt Inq Studio es aus derselben Schrift, sofern sie auf Ihrem Computer installiert ist; nur
wenn sie nirgends zu finden ist, wird das fehlende Zeichen in einer ähnlichen Schrift gesetzt, und ein Hinweis
sagt es Ihnen. Der Tab **Schriften** der Seitenleiste zeigt für jede Schrift, was beim Bearbeiten möglich ist.

**Text hinzufügen** (`T`): dorthin klicken, wo er beginnen soll, tippen, mit `Strg+Eingabe` bestätigen.

## Bilder

Mit dem Bearbeiten-Werkzeug ein Bild anklicken: Ziehen verschiebt es, Ziehen an der Ecke skaliert es; im
Inspektor lässt es sich ersetzen, speichern oder löschen. **Bild einfügen** (`I`) setzt ein PNG oder JPEG.

## Kommentare und Formen

Hervorheben (`M`), Unterstreichen (`U`) und Durchstreichen (`K`) durch Ziehen über Text. Notiz (`N`),
Zeichnen (`D`), Rechteck, Ellipse, Linie, Pfeil, Vieleck, Textfeld (`B`), Stempel (`S`) und Links (`L`) durch
Klicken oder Ziehen auf der Seite. Mit dem Auswählen-Werkzeug eine Anmerkung anklicken, um sie im Inspektor
zu ändern, oder ziehen, um sie zu verschieben.

## Formulare

Formularfelder werden mit dem Auswählen-Werkzeug direkt auf der Seite ausgefüllt. *Dokumentwerkzeuge ›
Formularfelder einbetten* macht aus den ausgefüllten Werten gewöhnlichen Seiteninhalt.

## Seiten

Im Tab **Seiten**: Miniaturen ziehen, um umzuordnen; Rechtsklick für Drehen, Duplizieren, leere Seite
einfügen, Seiten aus einem anderen PDF einfügen, Extrahieren und Löschen. Die *Dokumentwerkzeuge* ergänzen
Wasserzeichen, Kopf- und Fußzeilen, Bates-Nummern, Seitenbezeichnungen, das Ändern auf ein Papierformat und
das Teilen in mehrere Dateien. **Zuschneiden** (`C`) blendet alles außerhalb eines Bereichs aus.

## Schwärzen

**Schwärzen** (`X`) ist für Inhalte, die verschwinden müssen, nicht nur abgedeckt werden sollen. Darüber
ziehen — oder mit *Alle markieren* jedes Vorkommen eines Wortes finden lassen —, die Markierungen prüfen
und im Inspektor anwenden. Text, Bildpunkte und Zeichnungen unter jeder Markierung werden aus der Datei
selbst entfernt. Nach dem Speichern lassen sie sich nicht zurückholen.

## Scans: Text erkennen (OCR)

*Dokumentwerkzeuge › Text erkennen*. Sprache des Textes und Seiten wählen. Die Wörter werden unsichtbar
über den Scan gelegt, sodass man ihn durchsuchen, markieren und kopieren kann; das Bild bleibt, wie es ist.
Die Erkennung läuft auf Ihrem Computer mit den in Windows installierten OCR-Sprachen (Einstellungen › Zeit
und Sprache › Sprache und Region › Ihre Sprache › Sprachoptionen).

## Signaturen

*Dokumentwerkzeuge › Signaturen* prüft jede Signatur im Dokument und sagt in klaren Worten:

- ob der signierte Inhalt **unverändert** ist und ob der Datei danach etwas hinzugefügt wurde;
- **wer unterzeichnet hat**, laut Zertifikat, und ob Ihr Computer der ausstellenden Stelle vertraut.

*Signieren…* signiert das Dokument mit einer digitalen ID:

- **Aus Windows**: ein Zertifikat aus Ihrem Windows-Zertifikatspeicher, auch von Smartcards und Token.
- **Aus einer Datei**: eine `.pfx`- oder `.p12`-Datei mit ihrem Kennwort.
- **Neu erstellen**: eine selbstsignierte ID, gespeichert als `.pfx`. Sie belegt, dass ein Dokument seit
  Ihrer Signatur nicht verändert wurde, aber nicht, wer Sie sind — dafür braucht es ein Zertifikat eines
  Vertrauensdiensteanbieters.

Eine Signatur kann auf einer Seite sichtbar sein (Name, Datum, Grund) oder unsichtbar. Ist in den
Einstellungen ein **Zeitstempelserver** eingetragen, trägt die Signatur zusätzlich die Zeit eines
Zeitstempeldienstes; so bleibt sie prüfbar, nachdem Ihr Zertifikat abgelaufen ist.

Beim Signieren wird das Dokument gespeichert. Spätere Änderungen werden an die Datei angehängt; die
Signatur bleibt gültig und zeigt, dass nach ihr etwas hinzugekommen ist.

## Kennwortschutz

*Dokumentwerkzeuge › Mit Kennwort schützen*. Ein **Kennwort zum Öffnen** sperrt das Dokument. Ein
**Berechtigungskennwort** mit Einschränkungen (kein Drucken, kein Kopieren …) bittet Anzeigeprogramme,
sich zurückzuhalten; es ist eine Bitte, kein Schloss. Ein geschütztes Dokument, das Sie öffnen und speichern,
behält seinen Schutz.

## Kleinere Dateien, Archivierung, Vergleich

- **Optimieren** verringert die Auflösung von Bildern, die mehr haben als nötig, führt doppelte Schriften und
  Bilder zusammen und zeigt die Größe vorher und nachher.
- **PDF/A** prüft das Dokument gegen die Archivstandards PDF/A-1b, 2b und 3b, listet auf, was nicht
  entspricht, und wandelt um, was sich beheben lässt. Die Prüfungen decken die Regeln ab, an denen Dokumente
  in der Praxis scheitern; sie sind keine zertifizierte Validierung.
- **Vergleichen mit…** zeigt zwei Dokumente nebeneinander: entfernte Wörter rot, hinzugefügte grün, dazu
  eine Liste der Änderungen. *Unterschiede im Bild markieren* findet auch geänderte Bilder und Grafiken.

## Barrierefreiheit

*Dokumentwerkzeuge › Barrierefreiheit* listet auf, worüber Nutzer von Screenreadern stolpern, setzt die
Dokumentsprache, gibt Abbildungen Alternativtext und kann die Lesereihenfolge über die Seite zeichnen.

## Exportieren und Drucken

*Dokumentwerkzeuge › Seiten oder Text exportieren* speichert Seiten als PNG oder JPEG in wählbarer Auflösung
oder den Text als Textdatei. `Strg+P` druckt.

## Einstellungen

Sprache (Englisch, Deutsch; gilt nach einem Neustart), Design, Zoom, Einheiten, Ihr Verfassername, die Art
des Speicherns, der Zeitstempelserver, Updates und der Ort der Protokolldatei. Ihre Dokumente bleiben auf
Ihrem Computer: Inq Studio lädt nichts hoch. Die einzigen Verbindungen sind die tägliche Prüfung auf eine
neue Version und, falls eingetragen, der Zeitstempelserver — der einen Hashwert erhält, nie das Dokument.

Meldungen aus der PDF-Engine (manche Fehlertexte und Hinweise) erscheinen teilweise noch auf Englisch.

## Tastenkürzel

| | |
| --- | --- |
| `Strg+O` / `Strg+S` / `Strg+Umschalt+S` | Öffnen / Speichern / Speichern unter |
| `Strg+Z` / `Strg+Y` | Rückgängig / Wiederholen |
| `Strg+F` / `Strg+P` | Suchen / Drucken |
| `Strg+B` / `Strg+I` / `Strg+,` | Seitenleiste / Inspektor / Einstellungen |
| `Strg+W` / `Strg+Tab` | Tab schließen / nächster Tab |
| `Strg+=` / `Strg+-` / `Strg+0` | Vergrößern / Verkleinern / Breite anpassen |
| `V` `H` `E` `T` `I` | Auswählen, Hand, Bearbeiten, Text hinzufügen, Bild einfügen |
| `M` `U` `K` `N` `D` `B` `S` `L` | Hervorheben, Unterstreichen, Durchstreichen, Notiz, Zeichnen, Textfeld, Stempel, Links |
| `X` `C` | Schwärzen, Zuschneiden |
