# ragnar-calc

Calculator for the Ragnar desktop — session component shipped in
`ragnar-core`, so a complete installation always includes it.

*Deutsche Version: [README_de.md](README_de.md)*

## Modes

- **Standard**: digits, `+ − × ÷`, percent, `±`, `x²`, `√`, `1/x`,
  clear/backspace — the everyday set.
- **Scientific** ("Erweitert" header toggle): parentheses,
  `sin cos tan` + inverses, `ln`, `log`, `eˣ`, `10ˣ`, `xʸ`, `mod`,
  `n!`, `|x|`, `π`, `e`, plus a `DEG/RAD` unit switch.

## Behaviour

- Buttons edit the entry text; `=` or Enter evaluates it — typed
  expressions (`3*(4+sqrt(2))/7`) and button input are equivalent.
- Own recursive-descent parser (`src/parser.rs`): correct
  precedence, right-associative `^`, postfix `%`/`!`, comma or dot
  decimals, domain errors shown as "Fehler".

GPL-3.0-or-later — Jörg Schröder / LieraOS Project
