//! Equation script to LaTeX conversion for HWP documents.
//!
//! HWP equations use a script-based DSL similar to MathType's equation editor.
//! This module converts HWP equation scripts to LaTeX format.

use std::iter::Peekable;
use std::str::Chars;

/// Converts an HWP equation script to LaTeX format.
pub fn to_latex(script: &str) -> String {
    let converter = EquationConverter::new(script);
    converter.convert()
}

/// Equation converter that parses HWP equation scripts.
struct EquationConverter<'a> {
    chars: Peekable<Chars<'a>>,
    output: String,
}

impl<'a> EquationConverter<'a> {
    fn new(script: &'a str) -> Self {
        Self {
            chars: script.chars().peekable(),
            output: String::new(),
        }
    }

    fn convert(mut self) -> String {
        while self.chars.peek().is_some() {
            self.parse_element();
        }
        self.output
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn next(&mut self) -> Option<char> {
        self.chars.next()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.next();
        }
    }

    fn parse_element(&mut self) {
        self.skip_whitespace();

        match self.peek() {
            None => {}
            Some('{') => {
                self.next();
                self.output.push('{');
                while self.peek().is_some() && self.peek() != Some('}') {
                    self.parse_element();
                }
                if self.peek() == Some('}') {
                    self.next();
                    self.output.push('}');
                }
            }
            Some('}') => {
                self.next();
                self.output.push('}');
            }
            Some('^') => {
                self.next();
                self.output.push('^');
            }
            Some('_') => {
                self.next();
                self.output.push('_');
            }
            Some('+') | Some('-') | Some('=') | Some('(') | Some(')') | Some('[') | Some(']')
            | Some(',') | Some('.') | Some('!') | Some('?') | Some(':') | Some(';') => {
                let ch = self.next().unwrap();
                self.output.push(ch);
            }
            Some(c) if c.is_ascii_digit() => {
                let ch = self.next().unwrap();
                self.output.push(ch);
            }
            Some(c) if c.is_ascii_alphabetic() => {
                let word = self.read_word();
                self.process_keyword(&word);
            }
            Some(_) => {
                // Pass through other characters
                let ch = self.next().unwrap();
                self.output.push(ch);
            }
        }
    }

    fn read_word(&mut self) -> String {
        let mut word = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() {
                word.push(self.next().unwrap());
            } else {
                break;
            }
        }
        word
    }

    fn read_group(&mut self) -> String {
        self.skip_whitespace();

        if self.peek() == Some('{') {
            self.next(); // consume '{'
            let mut depth = 1;
            let mut group = String::new();
            while let Some(c) = self.next() {
                match c {
                    '{' => {
                        depth += 1;
                        group.push(c);
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        group.push(c);
                    }
                    _ => group.push(c),
                }
            }
            // Recursively convert the group content
            to_latex(&group)
        } else {
            // Read a single word or character
            self.read_word()
        }
    }

    fn process_keyword(&mut self, word: &str) {
        match word.to_uppercase().as_str() {
            // Fractions
            "OVER" => {
                // a OVER b → \frac{a}{b}
                // The numerator was already output, we need to wrap it
                // For simplicity, just output \frac notation
                self.output.push_str(" / ");
            }

            // Roots
            "SQRT" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\sqrt{{{}}}", arg));
            }
            "ROOT" => {
                // ROOT n OF expr → \sqrt[n]{expr}
                let n = self.read_group();
                self.skip_whitespace();
                // Skip "OF" if present
                if self.peek() == Some('O') || self.peek() == Some('o') {
                    let kw = self.read_word();
                    if kw.to_uppercase() != "OF" {
                        // Put it back somehow - for now just include it
                    }
                }
                let expr = self.read_group();
                self.output.push_str(&format!("\\sqrt[{}]{{{}}}", n, expr));
            }

            // Integrals
            "INT" => {
                self.output.push_str("\\int");
            }
            "IINT" => {
                self.output.push_str("\\iint");
            }
            "IIINT" => {
                self.output.push_str("\\iiint");
            }
            "OINT" => {
                self.output.push_str("\\oint");
            }
            "OIINT" => {
                self.output.push_str("\\oiint");
            }

            // Summation and products
            "SUM" => {
                self.output.push_str("\\sum");
            }
            "PROD" => {
                self.output.push_str("\\prod");
            }
            "LIM" => {
                self.output.push_str("\\lim");
            }
            "LIMSUP" => {
                self.output.push_str("\\limsup");
            }
            "LIMINF" => {
                self.output.push_str("\\liminf");
            }

            // Trigonometric functions
            "SIN" => self.output.push_str("\\sin"),
            "COS" => self.output.push_str("\\cos"),
            "TAN" => self.output.push_str("\\tan"),
            "COT" => self.output.push_str("\\cot"),
            "SEC" => self.output.push_str("\\sec"),
            "CSC" => self.output.push_str("\\csc"),
            "SINH" => self.output.push_str("\\sinh"),
            "COSH" => self.output.push_str("\\cosh"),
            "TANH" => self.output.push_str("\\tanh"),
            "COTH" => self.output.push_str("\\coth"),
            "ARCSIN" => self.output.push_str("\\arcsin"),
            "ARCCOS" => self.output.push_str("\\arccos"),
            "ARCTAN" => self.output.push_str("\\arctan"),

            // Logarithms
            "LOG" => self.output.push_str("\\log"),
            "LN" => self.output.push_str("\\ln"),
            "EXP" => self.output.push_str("\\exp"),

            // Greek letters - lowercase
            "ALPHA" => self.output.push_str("\\alpha"),
            "BETA" => self.output.push_str("\\beta"),
            "GAMMA" => self.output.push_str("\\gamma"),
            "DELTA" => self.output.push_str("\\delta"),
            "EPSILON" => self.output.push_str("\\epsilon"),
            "VAREPSILON" => self.output.push_str("\\varepsilon"),
            "ZETA" => self.output.push_str("\\zeta"),
            "ETA" => self.output.push_str("\\eta"),
            "THETA" => self.output.push_str("\\theta"),
            "VARTHETA" => self.output.push_str("\\vartheta"),
            "IOTA" => self.output.push_str("\\iota"),
            "KAPPA" => self.output.push_str("\\kappa"),
            "LAMBDA" => self.output.push_str("\\lambda"),
            "MU" => self.output.push_str("\\mu"),
            "NU" => self.output.push_str("\\nu"),
            "XI" => self.output.push_str("\\xi"),
            "OMICRON" => self.output.push_str("o"),
            "PI" => self.output.push_str("\\pi"),
            "VARPI" => self.output.push_str("\\varpi"),
            "RHO" => self.output.push_str("\\rho"),
            "VARRHO" => self.output.push_str("\\varrho"),
            "SIGMA" => self.output.push_str("\\sigma"),
            "VARSIGMA" => self.output.push_str("\\varsigma"),
            "TAU" => self.output.push_str("\\tau"),
            "UPSILON" => self.output.push_str("\\upsilon"),
            "PHI" => self.output.push_str("\\phi"),
            "VARPHI" => self.output.push_str("\\varphi"),
            "CHI" => self.output.push_str("\\chi"),
            "PSI" => self.output.push_str("\\psi"),
            "OMEGA" => self.output.push_str("\\omega"),

            // Greek letters - uppercase
            "UALPHA" | "ALPHA2" => self.output.push_str("A"),
            "UBETA" | "BETA2" => self.output.push_str("B"),
            "UGAMMA" | "GAMMA2" => self.output.push_str("\\Gamma"),
            "UDELTA" | "DELTA2" => self.output.push_str("\\Delta"),
            "UEPSILON" | "EPSILON2" => self.output.push_str("E"),
            "UZETA" | "ZETA2" => self.output.push_str("Z"),
            "UETA" | "ETA2" => self.output.push_str("H"),
            "UTHETA" | "THETA2" => self.output.push_str("\\Theta"),
            "UIOTA" | "IOTA2" => self.output.push_str("I"),
            "UKAPPA" | "KAPPA2" => self.output.push_str("K"),
            "ULAMBDA" | "LAMBDA2" => self.output.push_str("\\Lambda"),
            "UMU" | "MU2" => self.output.push_str("M"),
            "UNU" | "NU2" => self.output.push_str("N"),
            "UXI" | "XI2" => self.output.push_str("\\Xi"),
            "UOMICRON" | "OMICRON2" => self.output.push_str("O"),
            "UPI" | "PI2" => self.output.push_str("\\Pi"),
            "URHO" | "RHO2" => self.output.push_str("P"),
            "USIGMA" | "SIGMA2" => self.output.push_str("\\Sigma"),
            "UTAU" | "TAU2" => self.output.push_str("T"),
            "UUPSILON" | "UPSILON2" => self.output.push_str("\\Upsilon"),
            "UPHI" | "PHI2" => self.output.push_str("\\Phi"),
            "UCHI" | "CHI2" => self.output.push_str("X"),
            "UPSI" | "PSI2" => self.output.push_str("\\Psi"),
            "UOMEGA" | "OMEGA2" => self.output.push_str("\\Omega"),

            // Operators and relations (include trailing space for proper spacing)
            "TIMES" => self.output.push_str("\\times "),
            "DIV" => self.output.push_str("\\div "),
            "CDOT" => self.output.push_str("\\cdot "),
            "PM" | "PLUSMINUS" => self.output.push_str("\\pm "),
            "MP" | "MINUSPLUS" => self.output.push_str("\\mp "),
            "LEQ" | "LE" => self.output.push_str("\\leq "),
            "GEQ" | "GE" => self.output.push_str("\\geq "),
            "NEQ" | "NE" => self.output.push_str("\\neq "),
            "APPROX" => self.output.push_str("\\approx "),
            "EQUIV" => self.output.push_str("\\equiv "),
            "SIM" => self.output.push_str("\\sim "),
            "SIMEQ" => self.output.push_str("\\simeq "),
            "CONG" => self.output.push_str("\\cong "),
            "PROPTO" => self.output.push_str("\\propto "),
            "SUBSET" => self.output.push_str("\\subset "),
            "SUPSET" => self.output.push_str("\\supset "),
            "SUBSETEQ" => self.output.push_str("\\subseteq "),
            "SUPSETEQ" => self.output.push_str("\\supseteq "),
            "IN" => self.output.push_str("\\in "),
            "NI" | "OWNS" => self.output.push_str("\\ni "),
            "NOTIN" => self.output.push_str("\\notin "),

            // Set theory
            "EMPTYSET" => self.output.push_str("\\emptyset"),
            "CUP" | "UNION" => self.output.push_str("\\cup "),
            "CAP" | "INTER" | "INTERSECTION" => self.output.push_str("\\cap "),
            "SETMINUS" => self.output.push_str("\\setminus "),
            "BIGCUP" => self.output.push_str("\\bigcup "),
            "BIGCAP" => self.output.push_str("\\bigcap "),

            // Logic (binary operators need trailing space)
            "LAND" | "AND" => self.output.push_str("\\land "),
            "LOR" | "OR" => self.output.push_str("\\lor "),
            "LNOT" | "NOT" => self.output.push_str("\\lnot "),
            "FORALL" => self.output.push_str("\\forall "),
            "EXISTS" => self.output.push_str("\\exists "),
            "NEXISTS" => self.output.push_str("\\nexists "),
            "IMPLIES" | "RIGHTARROW" => self.output.push_str("\\Rightarrow "),
            "IFF" | "LEFTRIGHTARROW" => self.output.push_str("\\Leftrightarrow "),

            // Arrows
            "LEFTARROW" | "LARROW" => self.output.push_str("\\leftarrow "),
            "RARROW" => self.output.push_str("\\rightarrow "),
            "UPARROW" => self.output.push_str("\\uparrow "),
            "DOWNARROW" => self.output.push_str("\\downarrow "),
            "LRARROW" => self.output.push_str("\\leftrightarrow "),
            "MAPSTO" => self.output.push_str("\\mapsto "),

            // Brackets and delimiters
            "LBRACE" => self.output.push_str("\\{"),
            "RBRACE" => self.output.push_str("\\}"),
            "LANGLE" => self.output.push_str("\\langle"),
            "RANGLE" => self.output.push_str("\\rangle"),
            "LFLOOR" => self.output.push_str("\\lfloor"),
            "RFLOOR" => self.output.push_str("\\rfloor"),
            "LCEIL" => self.output.push_str("\\lceil"),
            "RCEIL" => self.output.push_str("\\rceil"),
            "VERT" | "BAR" => self.output.push_str("|"),
            "DVERT" | "DBAR" => self.output.push_str("\\|"),

            // Dots
            "LDOTS" | "CDOTS" | "DOTS" => self.output.push_str("\\cdots"),
            "VDOTS" => self.output.push_str("\\vdots"),
            "DDOTS" => self.output.push_str("\\ddots"),

            // Special symbols
            "INFTY" | "INF" => self.output.push_str("\\infty"),
            "PARTIAL" => self.output.push_str("\\partial"),
            "NABLA" => self.output.push_str("\\nabla"),
            "PRIME" => self.output.push_str("'"),
            "DPRIME" => self.output.push_str("''"),
            "DEGREE" => self.output.push_str("^\\circ"),
            "ANGLE" => self.output.push_str("\\angle"),
            "PERP" => self.output.push_str("\\perp"),
            "PARALLEL" => self.output.push_str("\\parallel"),

            // Matrix
            "MATRIX" => {
                // MATRIX { row1 # row2 # ... } where columns are separated by &
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    self.next();
                    let mut content = String::new();
                    let mut depth = 1;
                    while let Some(c) = self.next() {
                        match c {
                            '{' => {
                                depth += 1;
                                content.push(c);
                            }
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                content.push(c);
                            }
                            '#' => content.push_str(" \\\\ "),
                            '&' => content.push_str(" & "),
                            _ => content.push(c),
                        }
                    }
                    let inner = to_latex(&content);
                    self.output.push_str(&format!("\\begin{{pmatrix}} {} \\end{{pmatrix}}", inner));
                }
            }

            "PMATRIX" => {
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    let content = self.read_group();
                    let inner = content.replace('#', " \\\\ ").replace('&', " & ");
                    self.output.push_str(&format!("\\begin{{pmatrix}} {} \\end{{pmatrix}}", inner));
                }
            }

            "BMATRIX" => {
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    let content = self.read_group();
                    let inner = content.replace('#', " \\\\ ").replace('&', " & ");
                    self.output.push_str(&format!("\\begin{{bmatrix}} {} \\end{{bmatrix}}", inner));
                }
            }

            "VMATRIX" => {
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    let content = self.read_group();
                    let inner = content.replace('#', " \\\\ ").replace('&', " & ");
                    self.output.push_str(&format!("\\begin{{vmatrix}} {} \\end{{vmatrix}}", inner));
                }
            }

            // Accents and decorations
            "HAT" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\hat{{{}}}", arg));
            }
            "OVERLINE" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\overline{{{}}}", arg));
            }
            "VEC" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\vec{{{}}}", arg));
            }
            "DOT" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\dot{{{}}}", arg));
            }
            "DDOT" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\ddot{{{}}}", arg));
            }
            "TILDE" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\tilde{{{}}}", arg));
            }
            "WIDETILDE" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\widetilde{{{}}}", arg));
            }
            "WIDEHAT" => {
                let arg = self.read_group();
                self.output.push_str(&format!("\\widehat{{{}}}", arg));
            }

            // Superscript/subscript keywords
            "SUP" => {
                self.output.push('^');
            }
            "SUB" => {
                self.output.push('_');
            }

            // Fraction using FRAC keyword
            "FRAC" => {
                let num = self.read_group();
                let den = self.read_group();
                self.output.push_str(&format!("\\frac{{{}}}{{{}}}", num, den));
            }

            // Cases
            "CASES" => {
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    let content = self.read_group();
                    let inner = content.replace('#', " \\\\ ");
                    self.output.push_str(&format!("\\begin{{cases}} {} \\end{{cases}}", inner));
                }
            }

            // Other common functions
            "DET" => self.output.push_str("\\det"),
            "DIM" => self.output.push_str("\\dim"),
            "GCD" => self.output.push_str("\\gcd"),
            "HOM" => self.output.push_str("\\hom"),
            "INF2" => self.output.push_str("\\inf"),
            "KER" => self.output.push_str("\\ker"),
            "MAX" => self.output.push_str("\\max"),
            "MIN" => self.output.push_str("\\min"),
            "MOD" => self.output.push_str("\\mod"),
            "SUP2" => self.output.push_str("\\sup"),

            // Default: treat as regular text/variable
            _ => {
                // Single letter variables stay as-is, longer words get wrapped
                if word.len() == 1 {
                    self.output.push_str(word);
                } else {
                    // Could be a variable name, output as-is
                    self.output.push_str(word);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_variable() {
        assert_eq!(to_latex("x"), "x");
    }

    #[test]
    fn test_greek_letters() {
        assert_eq!(to_latex("ALPHA"), "\\alpha");
        assert_eq!(to_latex("OMEGA"), "\\omega");
        assert_eq!(to_latex("GAMMA2"), "\\Gamma");
    }

    #[test]
    fn test_sqrt() {
        assert_eq!(to_latex("SQRT{x}"), "\\sqrt{x}");
        assert_eq!(to_latex("SQRT{x+y}"), "\\sqrt{x+y}");
    }

    #[test]
    fn test_trig_functions() {
        assert_eq!(to_latex("SIN{x}"), "\\sin{x}");
        assert_eq!(to_latex("COS THETA"), "\\cos\\theta");
    }

    #[test]
    fn test_operators() {
        assert_eq!(to_latex("a TIMES b"), "a\\times b");
        assert_eq!(to_latex("x LEQ y"), "x\\leq y");
    }

    #[test]
    fn test_integral() {
        assert_eq!(to_latex("INT"), "\\int");
        assert_eq!(to_latex("OINT"), "\\oint");
    }

    #[test]
    fn test_infinity() {
        assert_eq!(to_latex("INFTY"), "\\infty");
    }

    #[test]
    fn test_matrix() {
        let result = to_latex("MATRIX{a & b # c & d}");
        assert!(result.contains("\\begin{pmatrix}"));
        assert!(result.contains("\\end{pmatrix}"));
    }

    #[test]
    fn test_frac() {
        assert_eq!(to_latex("FRAC{a}{b}"), "\\frac{a}{b}");
    }

    #[test]
    fn test_hat_vec() {
        assert_eq!(to_latex("HAT{x}"), "\\hat{x}");
        assert_eq!(to_latex("VEC{v}"), "\\vec{v}");
    }
}
