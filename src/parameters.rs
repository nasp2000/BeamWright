//! Named parameters with expressions and optional unit labels.
//!
//! An expression is a number, a parameter name or an arithmetic combination of
//! both with `+`, `-`, `*`, `/` and parentheses. Units are labels for the reader;
//! the arithmetic itself is unitless, so adding a length to a speed is not
//! detected. The evaluator rejects unknown names, cycles and non-finite results.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One named parameter stored with the document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub expression: String,
    /// Display label such as `mm`, `mm/min`, `%` or `deg`.
    #[serde(default)]
    pub unit: String,
}

impl Parameter {
    pub fn new(name: impl Into<String>, expression: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            expression: expression.into(),
            unit: String::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("a parameter needs a name".to_string());
        }
        if !is_identifier(self.name.trim()) {
            return Err(format!(
                "parameter names use letters, digits and underscores: {}",
                self.name
            ));
        }
        if self.expression.trim().is_empty() {
            return Err(format!("parameter '{}' has no expression", self.name));
        }
        Ok(())
    }
}

/// Validate every parameter and evaluate it, catching duplicates and cycles.
pub fn validate(parameters: &[Parameter]) -> Result<(), String> {
    let mut names = std::collections::HashSet::new();
    for parameter in parameters {
        parameter.validate()?;
        let name = parameter.name.trim();
        if !names.insert(name) {
            return Err(format!("duplicate parameter: {name}"));
        }
    }
    for parameter in parameters {
        evaluate(&parameter.expression, parameters)
            .map_err(|error| format!("parameter '{}': {error}", parameter.name))?;
    }
    Ok(())
}

/// Evaluate one expression with the given parameters.
pub fn evaluate(expression: &str, parameters: &[Parameter]) -> Result<f64, String> {
    let mut visiting = Vec::new();
    evaluate_with(expression, parameters, &mut visiting)
}

fn evaluate_with(
    expression: &str,
    parameters: &[Parameter],
    visiting: &mut Vec<String>,
) -> Result<f64, String> {
    let tokens = tokenize(expression)?;
    if tokens.is_empty() {
        return Err("the expression is empty".to_string());
    }
    let mut evaluator = Evaluator {
        tokens,
        position: 0,
        parameters,
        visiting,
    };
    let value = evaluator.expression()?;
    if evaluator.position != evaluator.tokens.len() {
        return Err("unexpected text after the expression".to_string());
    }
    if !value.is_finite() {
        return Err("the result is not a finite number".to_string());
    }
    Ok(value)
}

/// Evaluate a named parameter.
pub fn value_of(parameters: &[Parameter], name: &str) -> Result<f64, String> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.name == name)
        .ok_or_else(|| format!("unknown parameter: {name}"))?;
    evaluate(&parameter.expression, parameters)
}

/// Every parameter with its value or error, in document order.
pub fn resolved(parameters: &[Parameter]) -> Vec<(String, Result<f64, String>)> {
    parameters
        .iter()
        .map(|parameter| {
            (
                parameter.name.clone(),
                evaluate(&parameter.expression, parameters),
            )
        })
        .collect()
}

/// Read an optional expression from a field map.
pub fn field(
    expressions: &BTreeMap<String, String>,
    key: &str,
    parameters: &[Parameter],
    fallback: f64,
) -> Result<f64, String> {
    match expressions.get(key) {
        None => Ok(fallback),
        Some(expression) => {
            evaluate(expression, parameters).map_err(|error| format!("field '{key}': {error}"))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Open,
    Close,
}

fn tokenize(source: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut characters = source.chars().peekable();
    while let Some(character) = characters.next() {
        if character.is_whitespace() {
            continue;
        }
        match character {
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => tokens.push(Token::Star),
            '/' => tokens.push(Token::Slash),
            '(' => tokens.push(Token::Open),
            ')' => tokens.push(Token::Close),
            character if character.is_ascii_digit() || character == '.' => {
                let mut number = String::from(character);
                while let Some(&next) = characters.peek() {
                    if next.is_ascii_digit() || next == '.' {
                        number.push(next);
                        characters.next();
                    } else {
                        break;
                    }
                }
                let value: f64 = number
                    .parse()
                    .map_err(|_| format!("invalid number: {number}"))?;
                tokens.push(Token::Number(value));
            }
            character if character.is_alphabetic() || character == '_' => {
                let mut name = String::from(character);
                while let Some(&next) = characters.peek() {
                    if next.is_ascii_alphanumeric() || next == '_' {
                        name.push(next);
                        characters.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Name(name));
            }
            other => return Err(format!("unexpected character: {other}")),
        }
    }
    Ok(tokens)
}

fn is_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

struct Evaluator<'a> {
    tokens: Vec<Token>,
    position: usize,
    parameters: &'a [Parameter],
    visiting: &'a mut Vec<String>,
}

impl Evaluator<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn expression(&mut self) -> Result<f64, String> {
        let mut value = self.term()?;
        while let Some(token) = self.peek() {
            match token {
                Token::Plus => {
                    self.position += 1;
                    value += self.term()?;
                }
                Token::Minus => {
                    self.position += 1;
                    value -= self.term()?;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn term(&mut self) -> Result<f64, String> {
        let mut value = self.factor()?;
        while let Some(token) = self.peek() {
            match token {
                Token::Star => {
                    self.position += 1;
                    value *= self.factor()?;
                }
                Token::Slash => {
                    self.position += 1;
                    let divisor = self.factor()?;
                    if divisor.abs() < 1e-12 {
                        return Err("division by zero".to_string());
                    }
                    value /= divisor;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn factor(&mut self) -> Result<f64, String> {
        let Some(token) = self.tokens.get(self.position).cloned() else {
            return Err("the expression ends early".to_string());
        };
        self.position += 1;
        match token {
            Token::Number(value) => Ok(value),
            Token::Name(name) => self.parameter(&name),
            Token::Minus => Ok(-self.factor()?),
            Token::Open => {
                let value = self.expression()?;
                if self.tokens.get(self.position) != Some(&Token::Close) {
                    return Err("a parenthesis was not closed".to_string());
                }
                self.position += 1;
                Ok(value)
            }
            _ => Err("expected a number, a parameter or a parenthesis".to_string()),
        }
    }

    fn parameter(&mut self, name: &str) -> Result<f64, String> {
        if self.visiting.iter().any(|visited| visited == name) {
            return Err(format!("cycle through parameter '{name}'"));
        }
        let expression = self
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .map(|parameter| parameter.expression.clone())
            .ok_or_else(|| format!("unknown parameter: {name}"))?;
        self.visiting.push(name.to_string());
        let value = evaluate_with(&expression, self.parameters, self.visiting);
        self.visiting.pop();
        value
    }
}

#[cfg(test)]
mod tests {
    use super::{Parameter, evaluate, resolved, validate, value_of};

    fn parameters() -> Vec<Parameter> {
        vec![
            Parameter::new("thickness", "3"),
            Parameter::new("scale", "2"),
            Parameter::new("twice", "thickness * scale"),
        ]
    }

    #[test]
    fn arithmetic_respects_precedence_and_parentheses() {
        let parameters = parameters();
        assert!((evaluate("2 + 3 * 4", &parameters).unwrap() - 14.0).abs() < 1e-9);
        assert!((evaluate("(2 + 3) * 4", &parameters).unwrap() - 20.0).abs() < 1e-9);
        assert!((evaluate("-4 + 2", &parameters).unwrap() - -2.0).abs() < 1e-9);
        assert!((evaluate("10 / 4", &parameters).unwrap() - 2.5).abs() < 1e-9);
        assert!((evaluate("twice", &parameters).unwrap() - 6.0).abs() < 1e-9);
        assert!((evaluate("thickness * 2 + 1", &parameters).unwrap() - 7.0).abs() < 1e-9);
    }

    #[test]
    fn bad_expressions_are_rejected() {
        let parameters = parameters();
        assert!(evaluate("", &parameters).is_err());
        assert!(evaluate("2 +", &parameters).is_err());
        assert!(evaluate("(2 + 3", &parameters).is_err());
        assert!(evaluate("1 / 0", &parameters).is_err());
        assert!(evaluate("missing + 1", &parameters).is_err());
        assert!(evaluate("2 $ 3", &parameters).is_err());
        assert!(value_of(&parameters, "missing").is_err());
    }

    #[test]
    fn cycles_and_duplicates_are_rejected() {
        let cycle = vec![Parameter::new("a", "b + 1"), Parameter::new("b", "a + 1")];
        assert!(validate(&cycle).is_err());
        assert!(evaluate("a", &cycle).is_err());
        let duplicate = vec![Parameter::new("a", "1"), Parameter::new("a", "2")];
        assert!(validate(&duplicate).is_err());
        let self_reference = vec![Parameter::new("a", "a + 1")];
        assert!(validate(&self_reference).is_err());
    }

    #[test]
    fn names_and_units_are_validated() {
        let mut parameters = parameters();
        assert!(validate(&parameters).is_ok());
        parameters[0].unit = "mm".to_string();
        assert!(validate(&parameters).is_ok());
        parameters[0].name = "2 bad".to_string();
        assert!(validate(&parameters).is_err());
        parameters[0].name = "good".to_string();
        parameters[0].expression = "  ".to_string();
        assert!(validate(&parameters).is_err());
    }

    #[test]
    fn every_parameter_reports_its_value() {
        let values = resolved(&parameters());
        assert_eq!(values.len(), 3);
        assert!(values.iter().all(|(_, value)| value.is_ok()));
    }
}
