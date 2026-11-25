// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-calc — expression evaluator.
//
// Recursive-descent parser over the raw input text. Precedence,
// lowest to highest: + -  |  * / mod  |  unary -  |  ^ (right
// associative)  |  postfix % !  |  atoms. Scientific functions and
// the constants pi/e are evaluated here as well; trigonometric
// functions honour the caller's angle mode (degrees vs radians).

/// Whether sin/cos/tan interpret their argument as degrees.
#[derive(Clone, Copy, PartialEq)]
pub enum AngleMode {
    Radians,
    Degrees,
}

/// Evaluate an expression string. Returns Err("…") on syntax or
/// math errors — the caller shows a generic "Fehler" message, so
/// the text only matters for debugging.
pub fn eval(input: &str, mode: AngleMode) -> Result<f64, String> {
    let mut p = Parser {
        chars: input.chars().collect(),
        pos: 0,
        mode,
    };
    let v = p.expr()?;
    p.skip_ws();
    if p.pos != p.chars.len() {
        return Err(format!("trailing input at {}", p.pos));
    }
    if v.is_nan() {
        return Err("not a number".into());
    }
    Ok(v)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    mode: AngleMode,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ')) {
            self.pos += 1;
        }
    }

    /// Accept `ch` if it is next; also accepts the typographic
    /// variants the UI buttons insert (× ÷ −).
    fn eat(&mut self, ch: char) -> bool {
        self.skip_ws();
        let got = self.peek();
        let matches = got == Some(ch)
            || (ch == '*' && got == Some('×'))
            || (ch == '/' && got == Some('÷'))
            || (ch == '-' && got == Some('−'));
        if matches {
            self.pos += 1;
        }
        matches
    }

    /// expr := term (('+' | '-') term)*
    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                return Ok(v);
            }
        }
    }

    /// term := factor (('*' | '/' | 'mod') factor)*
    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.factor()?;
        loop {
            if self.eat('*') {
                v *= self.factor()?;
            } else if self.eat('/') {
                let d = self.factor()?;
                if d == 0.0 {
                    return Err("division by zero".into());
                }
                v /= d;
            } else if self.eat_word("mod") {
                let d = self.factor()?;
                if d == 0.0 {
                    return Err("modulo by zero".into());
                }
                v %= d;
            } else {
                return Ok(v);
            }
        }
    }

    /// factor := unary ('^' factor)*  — right-associative power.
    fn factor(&mut self) -> Result<f64, String> {
        let base = self.unary()?;
        if self.eat('^') {
            let exp = self.factor()?;
            let r = base.powf(exp);
            if r.is_nan() {
                return Err("invalid power".into());
            }
            return Ok(r);
        }
        Ok(base)
    }

    /// unary := '-' unary | '+' unary | postfix
    fn unary(&mut self) -> Result<f64, String> {
        if self.eat('-') {
            return Ok(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        self.postfix()
    }

    /// postfix := atom ('%' | '!')*  — percent and factorial.
    fn postfix(&mut self) -> Result<f64, String> {
        let mut v = self.atom()?;
        loop {
            if self.eat('%') {
                v /= 100.0;
            } else if self.eat('!') {
                if v < 0.0 || v.fract() != 0.0 || v > 170.0 {
                    return Err("factorial of non-integer or too large".into());
                }
                v = (1..=v as u64).product::<u64>() as f64;
            } else {
                return Ok(v);
            }
        }
    }

    /// atom := number | 'pi' | 'e' | func '(' expr ')' | '(' expr ')'
    fn atom(&mut self) -> Result<f64, String> {
        self.skip_ws();
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let v = self.expr()?;
                if !self.eat(')') {
                    return Err("missing ')'".into());
                }
                Ok(v)
            }
            Some(c) if c.is_ascii_alphabetic() || c == 'π' => self.named(),
            Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
            _ => Err(format!("unexpected input at {}", self.pos)),
        }
    }

    /// Consume an identifier/constant and either a function call or
    /// a bare constant.
    fn named(&mut self) -> Result<f64, String> {
        if self.eat('π') {
            return Ok(std::f64::consts::PI);
        }
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        match name.as_str() {
            "pi" => Ok(std::f64::consts::PI),
            "e" => Ok(std::f64::consts::E),
            "inf" | "infinity" => Ok(f64::INFINITY),
            _ => {
                self.skip_ws();
                if !self.eat('(') {
                    return Err(format!("unknown name '{name}'"));
                }
                let arg = self.expr()?;
                if !self.eat(')') {
                    return Err("missing ')'".into());
                }
                self.apply(&name, arg)
            }
        }
    }

    /// Consume a word operator like `mod` (needs a following operand,
    /// otherwise it is not consumed).
    fn eat_word(&mut self, word: &str) -> bool {
        self.skip_ws();
        let end = self.pos + word.chars().count();
        if end <= self.chars.len()
            && self.chars[self.pos..end].iter().collect::<String>() == word
        {
            self.pos = end;
            true
        } else {
            false
        }
    }

    /// Decimal number, both '.' and ',' accepted as separators.
    fn number(&mut self) -> Result<f64, String> {
        let start = self.pos;
        let mut seen_sep = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.pos += 1;
            } else if (c == '.' || c == ',') && !seen_sep {
                seen_sep = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(format!("expected number at {}", start));
        }
        let text: String = self.chars[start..self.pos]
            .iter()
            .map(|c| if *c == ',' { '.' } else { *c })
            .collect();
        text.parse::<f64>().map_err(|e| e.to_string())
    }

    /// Apply a known function name to its argument.
    fn apply(&self, name: &str, arg: f64) -> Result<f64, String> {
        let rad = if self.mode == AngleMode::Degrees {
            arg.to_radians()
        } else {
            arg
        };
        let r = match name {
            "sin" => rad.sin(),
            "cos" => rad.cos(),
            "tan" => rad.tan(),
            "asin" => {
                let a = arg.asin();
                if self.mode == AngleMode::Degrees { a.to_degrees() } else { a }
            }
            "acos" => {
                let a = arg.acos();
                if self.mode == AngleMode::Degrees { a.to_degrees() } else { a }
            }
            "atan" => {
                let a = arg.atan();
                if self.mode == AngleMode::Degrees { a.to_degrees() } else { a }
            }
            "ln" => arg.ln(),
            "log" => arg.log10(),
            "exp" => arg.exp(),
            "sqrt" => arg.sqrt(),
            "abs" => arg.abs(),
            "fact" => {
                if arg < 0.0 || arg.fract() != 0.0 || arg > 170.0 {
                    return Err("factorial of non-integer or too large".into());
                }
                (1..=arg as u64).product::<u64>() as f64
            }
            _ => return Err(format!("unknown function '{name}'")),
        };
        if r.is_nan() {
            return Err(format!("'{name}' out of domain"));
        }
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> f64 {
        eval(s, AngleMode::Degrees).expect(s)
    }

    #[test]
    fn arithmetic() {
        assert_eq!(ev("2+3*4"), 14.0);
        assert_eq!(ev("(2+3)*4"), 20.0);
        assert_eq!(ev("10/4"), 2.5);
        assert_eq!(ev("2^3^2"), 512.0); // right-assoc: 2^(3^2)
        assert_eq!(ev("-3+5"), 2.0);
        assert_eq!(ev("7 mod 3"), 1.0);
        assert_eq!(ev("50%"), 0.5);
        assert_eq!(ev("5!"), 120.0);
        assert_eq!(ev("1,5+1,5"), 3.0); // comma decimals
    }

    #[test]
    fn functions() {
        assert!((ev("sqrt(9)") - 3.0).abs() < 1e-9);
        assert!((ev("sin(90)") - 1.0).abs() < 1e-9); // degrees!
        assert!((ev("ln(e)") - 1.0).abs() < 1e-9);
        assert!((ev("log(1000)") - 3.0).abs() < 1e-9);
        assert!((ev("pi") - std::f64::consts::PI).abs() < 1e-12);
        assert!((ev("abs(-4)") - 4.0).abs() < 1e-9);
        assert!((ev("asin(1)") - 90.0).abs() < 1e-9); // degrees out
    }

    #[test]
    fn errors() {
        assert!(eval("1/0", AngleMode::Degrees).is_err());
        assert!(eval("sqrt(-1)", AngleMode::Degrees).is_err());
        assert!(eval("(1+2", AngleMode::Degrees).is_err());
        assert!(eval("abc", AngleMode::Degrees).is_err());
        assert!(eval("2.5!", AngleMode::Degrees).is_err());
    }
}
