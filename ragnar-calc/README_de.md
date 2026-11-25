# ragnar-calc

Taschenrechner für den Ragnar-Desktop — Session-Komponente in
`ragnar-core`, damit eine vollständige Installation ihn immer
mitliefert.

## Ansichten

- **Standard**: Ziffern, `+ − × ÷`, Prozent, `±`, `x²`, `√`, `1/x`,
  Löschen/Rücktaste — der Alltags-Satz.
- **Erweitert** (Header-Umschalter): Klammern, `sin cos tan` +
  Umkehrfunktionen, `ln`, `log`, `eˣ`, `10ˣ`, `xʸ`, `mod`, `n!`,
  `|x|`, `π`, `e`, plus `DEG/RAD`-Umschalter.

## Verhalten

- Tasten bearbeiten die Eingabezeile; `=` oder Enter wertet aus —
  getippte Ausdrücke (`3*(4+sqrt(2))/7`) und Tasteneingabe sind
  gleichwertig.
- Eigener Recursive-Descent-Parser (`src/parser.rs`): korrekte
  Rangfolge, rechtsassoziative `^`, Postfix-`%`/`!`, Komma oder
  Punkt als Dezimaltrenner, Definitionsfehler als „Fehler".

GPL-3.0-or-later — Jörg Schröder / LieraOS Project
