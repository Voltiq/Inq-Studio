# Inq Studio — Handbuch

Inq Studio ist ein PDF-Betrachter und -Editor für Windows. Dieses Handbuch geht durch, was er kann, in der
Reihenfolge, in der man es meist braucht. Derselbe Text öffnet sich in der App mit `F1`. *An English version
is in [GUIDE.md](GUIDE.md).*

## Installieren und aktualisieren

Das Installationsprogramm von der [Release-Seite](https://github.com/Voltiq/Inq-Studio/releases/latest)
herunterladen und ausführen. Es installiert für den aktuellen Benutzer und braucht keine
Administratorrechte. Es ist noch nicht signiert: Warnt SmartScreen, *Weitere Informationen › Trotzdem
ausführen* wählen.

Danach öffnet sich ein PDF über *Öffnen mit* im Explorer in Inq Studio; dort lässt es sich auch als
Standard festlegen.

Inq Studio sucht bei jedem Start nach einer neuen Version. Gibt es eine, fragt es, ob sie installiert werden
soll: **Jetzt aktualisieren**, **Später** (es fragt beim nächsten Start wieder) oder **Diese Version
überspringen** (es fragt erst bei der Version danach wieder). Mit *Beim Start nicht mehr fragen* erscheint
statt der Frage nur ein kurzer Hinweis; derselbe Schalter steht in den Einstellungen. *Einstellungen ›
Updates* prüft sofort und installiert. Updates sind signiert; die Signatur wird geprüft, bevor etwas installiert wird.

## Erste Schritte

1. Ein PDF öffnen: der Ordner-Knopf, `Strg+O`, oder Dateien irgendwo auf das Fenster ziehen.
2. Mit den Werkzeugen über der Seite lesen, kommentieren oder bearbeiten.
3. Mit `Strg+S` speichern, mit `Strg+Umschalt+S` unter neuem Namen.

Beim ersten Start zeigt eine kurze **Tour** die Teile des Fensters. Sie lässt sich über den Knopf **Hilfe**
(das Fragezeichen oben rechts) erneut starten; dort öffnet sich auch dieses Handbuch.

## Das Fenster

- **Titelleiste**: Symbol und Name der App, das geöffnete Dokument und die Knöpfe zum Minimieren,
  Maximieren und Schließen des Fensters. Ziehen verschiebt das Fenster, ein Doppelklick maximiert es.
- **Obere Leiste**: links Öffnen, Speichern, Rückgängig und Wiederholen; in der Mitte Name und Seite des
  Dokuments und seine Kennzeichen (*signiert*, Art des Kennwortschutzes, PDF/A-Angabe — ein Klick öffnet den
  passenden Dialog); rechts Design, **Dokumentwerkzeuge** (der Schraubenschlüssel), Hilfe, Einstellungen
  und Inspektor.
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

Ist in den Einstellungen **Text automatisch neu ausrichten** eingeschaltet (das ist die Vorgabe), behält ein
Absatz seine Form, wenn Sie eine seiner Zeilen ändern: Wird eine Zeile kürzer, rücken die Wörter nach, die
ihren Satz fortsetzen; wird sie länger, läuft der Überhang in die nächsten Zeilen; wird eine Zeile gelöscht,
rücken die Zeilen darunter nach oben. Es bewegen sich nur die Zeilen desselben Textblocks. Andere Blöcke und
der Rest der Seite bleiben, wo sie sind; ein Text, der um eine Zeile wächst, kann also in das laufen, was
darunter steht.

Was Sie tippen, wird **in der Schrift geschrieben, die das Dokument bereits verwendet**. Fehlt dieser Schrift
ein Zeichen, nimmt Inq Studio es aus derselben Schrift, sofern sie auf Ihrem Computer installiert ist; nur
wenn sie nirgends zu finden ist, wird das fehlende Zeichen in einer ähnlichen Schrift gesetzt, und ein Hinweis
sagt es Ihnen. Der Tab **Schriften** der Seitenleiste zeigt für jede Schrift, was beim Bearbeiten möglich ist.

**Text hinzufügen** (`T`): dorthin klicken, wo er beginnen soll, tippen, mit `Strg+Eingabe` bestätigen.

### Text auf Scans

Eine gescannte Seite ist ein Bild; ihre Wörter sind Bildpunkte, kein Text. Nach *Text erkennen* (siehe
unten) lassen sich die Wörter markieren und auch neu schreiben: mit dem Bearbeiten-Werkzeug auf eine
erkannte Zeile doppelklicken. Weil die alten Wörter Teil des Bildes sind, deckt Inq Studio die Zeile in der
Farbe des Papiers ab und zeichnet Ihren Text in der Farbe der Schrift darüber. Der neue Text wird in
Helvetica oder Arial gesetzt und trifft die Schrift des Scans daher nicht genau. Erkannter Text wird Zeile
für Zeile bearbeitet.

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

Die Erkennung läuft auf Ihrem Computer mit den in Windows installierten OCR-Sprachen. Das sind
Windows-Sprachpakete; mit der Sprache der Oberfläche von Inq Studio haben sie nichts zu tun.

- **Automatisch erkennen** (angeboten, wenn mehr als eine Sprache installiert ist) liest die erste Seite
  mit Text in jeder installierten Sprache und behält die passende.
- **Sprache zu Windows hinzufügen**: eine Sprache wählen und *Installieren…* drücken. Windows lädt das
  Paket herunter und fragt nach Administratorrechten; auf einem von Schule oder Firma verwalteten Computer
  ist das womöglich nicht erlaubt. Dasselbe geht in Windows unter *Einstellungen › Zeit und Sprache ›
  Sprache und Region*.

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

Sprache (Englisch, Deutsch, Spanisch, Französisch; gilt nach einem Neustart), Design, automatisches Ausrichten von Text, Zoom, Einheiten, Ihr Verfassername, die Art
des Speicherns, der Zeitstempelserver, Updates und der Ort der Protokolldatei. Ihre Dokumente bleiben auf
Ihrem Computer: Inq Studio lädt nichts hoch. Die einzigen Verbindungen sind die Prüfung auf eine neue
Version beim Start und, falls eingetragen, der Zeitstempelserver — der einen Hashwert erhält, nie das Dokument.

Meldungen aus der PDF-Engine (manche Fehlertexte und Hinweise) erscheinen teilweise noch auf Englisch.

## Wenn etwas nicht klappt

- **Ein PDF zeigt leere Seiten.** Auf die neueste Version aktualisieren; Dateien mancher Scanner wurden vor
  1.0.1 unvollständig gelesen. Bleibt es dabei, ist die Datei womöglich beschädigt: Andere Programme
  reparieren stillschweigend.
- **Eine Zeile lässt sich nicht bearbeiten.** Der Tab *Schriften* der Seitenleiste sagt für jede Schrift,
  was beim Bearbeiten möglich ist. Text, der Teil eines Bildes ist, braucht zuerst *Text erkennen*.
- **Im OCR-Dialog fehlt meine Sprache.** Ihr Paket mit *Sprache zu Windows hinzufügen* installieren.
- **Getippter Text ist auf einem Scan unsichtbar.** Auf 1.0.2 oder neuer aktualisieren.
- **Die App ist abgestürzt.** Der nächste Start zeigt den Absturzbericht. Protokolldatei und Berichte
  liegen nur auf Ihrem Computer (*Einstellungen › Diagnose › Im Ordner anzeigen*) und enthalten nie den
  Inhalt von Dokumenten. Hängen Sie sie an, wenn Sie ein Problem unter
  [github.com/Voltiq/Inq-Studio/issues](https://github.com/Voltiq/Inq-Studio/issues) melden.

## Tastenkürzel

| | |
| --- | --- |
| `Strg+O` / `Strg+S` / `Strg+Umschalt+S` | Öffnen / Speichern / Speichern unter |
| `Strg+Z` / `Strg+Y` | Rückgängig / Wiederholen |
| `Strg+F` / `Strg+P` | Suchen / Drucken |
| `Strg+B` / `Strg+I` / `Strg+,` | Seitenleiste / Inspektor / Einstellungen |
| `F1` | Dieses Handbuch |
| `Strg+W` / `Strg+Tab` | Tab schließen / nächster Tab |
| `Strg+=` / `Strg+-` / `Strg+0` | Vergrößern / Verkleinern / Breite anpassen |
| `V` `H` `E` `T` `I` | Auswählen, Hand, Bearbeiten, Text hinzufügen, Bild einfügen |
| `M` `U` `K` `N` `D` `B` `S` `L` | Hervorheben, Unterstreichen, Durchstreichen, Notiz, Zeichnen, Textfeld, Stempel, Links |
| `X` `C` | Schwärzen, Zuschneiden |
