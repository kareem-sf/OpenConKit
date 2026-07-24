//! Small, deterministic formula evaluator for documented BOQ-safe forms.
//!
//! This is intentionally not an Excel calculation engine. It supports
//! same-sheet arithmetic over numeric literals/cell references and one
//! `SUM(A1:B2)` range. Everything else is explicitly unverifiable.

use std::collections::BTreeMap;
use std::str::FromStr;

use openconkit_spreadsheet::{IngestedCell, IngestedSheet, NormalizedCellValue};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FormulaEvaluation {
    Value(Decimal),
    Unverifiable(&'static str),
}

pub(crate) fn evaluate(
    sheet: &IngestedSheet,
    raw_formula: &str,
    formula_cell: (u32, u32),
) -> FormulaEvaluation {
    let formula = raw_formula
        .trim()
        .strip_prefix('=')
        .unwrap_or(raw_formula.trim());
    if formula.is_empty() {
        return FormulaEvaluation::Unverifiable("empty_formula");
    }
    let numeric_cells: BTreeMap<(u32, u32), Decimal> = sheet
        .cells
        .iter()
        .filter(|cell| (cell.row, cell.column) != formula_cell)
        .filter_map(|cell| cell_decimal(cell).map(|value| ((cell.row, cell.column), value)))
        .collect();

    if formula
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("SUM("))
        && formula.ends_with(')')
    {
        return evaluate_sum(&numeric_cells, &formula[4..formula.len() - 1], formula_cell);
    }
    let mut parser = Parser::new(formula, &numeric_cells);
    match parser.expression() {
        Ok(value) if parser.at_end() => FormulaEvaluation::Value(value),
        Ok(_) => FormulaEvaluation::Unverifiable("unsupported_trailing_syntax"),
        Err(reason) => FormulaEvaluation::Unverifiable(reason),
    }
}

pub(crate) fn cell_decimal(cell: &IngestedCell) -> Option<Decimal> {
    match &cell.normalized_value {
        NormalizedCellValue::Integer(value) => Some(Decimal::from(*value)),
        NormalizedCellValue::Number(value) => Decimal::from_str(value).ok(),
        _ => None,
    }
}

fn evaluate_sum(
    cells: &BTreeMap<(u32, u32), Decimal>,
    argument: &str,
    formula_cell: (u32, u32),
) -> FormulaEvaluation {
    if argument.contains(',') || argument.contains(';') {
        return FormulaEvaluation::Unverifiable("multiple_sum_arguments");
    }
    let Some((start, end)) = argument.split_once(':') else {
        return FormulaEvaluation::Unverifiable("sum_requires_range");
    };
    let Some((start_row, start_column)) = parse_cell_ref(start) else {
        return FormulaEvaluation::Unverifiable("invalid_sum_start");
    };
    let Some((end_row, end_column)) = parse_cell_ref(end) else {
        return FormulaEvaluation::Unverifiable("invalid_sum_end");
    };
    if start_row > end_row || start_column > end_column {
        return FormulaEvaluation::Unverifiable("reversed_sum_range");
    }
    if (start_row..=end_row).contains(&formula_cell.0)
        && (start_column..=end_column).contains(&formula_cell.1)
    {
        return FormulaEvaluation::Unverifiable("self_reference");
    }
    let mut total = Decimal::ZERO;
    for row in start_row..=end_row {
        for column in start_column..=end_column {
            if let Some(value) = cells.get(&(row, column)) {
                total += *value;
            }
        }
    }
    FormulaEvaluation::Value(total)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Number(Decimal),
    Cell(u32, u32),
    Plus,
    Minus,
    Multiply,
    Divide,
    Open,
    Close,
}

struct Parser<'a> {
    tokens: Vec<Token>,
    position: usize,
    cells: &'a BTreeMap<(u32, u32), Decimal>,
    lex_error: Option<&'static str>,
}

impl<'a> Parser<'a> {
    fn new(formula: &str, cells: &'a BTreeMap<(u32, u32), Decimal>) -> Self {
        let (tokens, lex_error) = tokenize(formula);
        Self {
            tokens,
            position: 0,
            cells,
            lex_error,
        }
    }

    fn at_end(&self) -> bool {
        self.position == self.tokens.len()
    }

    fn expression(&mut self) -> Result<Decimal, &'static str> {
        if let Some(error) = self.lex_error {
            return Err(error);
        }
        let mut value = self.term()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.position += 1;
                    value += self.term()?;
                }
                Some(Token::Minus) => {
                    self.position += 1;
                    value -= self.term()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn term(&mut self) -> Result<Decimal, &'static str> {
        let mut value = self.factor()?;
        loop {
            match self.peek() {
                Some(Token::Multiply) => {
                    self.position += 1;
                    value = value
                        .checked_mul(self.factor()?)
                        .ok_or("arithmetic_overflow")?;
                }
                Some(Token::Divide) => {
                    self.position += 1;
                    let divisor = self.factor()?;
                    if divisor.is_zero() {
                        return Err("division_by_zero");
                    }
                    value = value.checked_div(divisor).ok_or("arithmetic_overflow")?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn factor(&mut self) -> Result<Decimal, &'static str> {
        match self.next() {
            Some(Token::Number(value)) => Ok(value),
            Some(Token::Cell(row, column)) => self
                .cells
                .get(&(row, column))
                .copied()
                .ok_or("referenced_cell_not_numeric"),
            Some(Token::Minus) => Decimal::ZERO
                .checked_sub(self.factor()?)
                .ok_or("arithmetic_overflow"),
            Some(Token::Plus) => self.factor(),
            Some(Token::Open) => {
                let value = self.expression()?;
                if self.next() != Some(Token::Close) {
                    return Err("unclosed_parenthesis");
                }
                Ok(value)
            }
            _ => Err("expected_operand"),
        }
    }

    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.position).copied()
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.peek();
        if token.is_some() {
            self.position += 1;
        }
        token
    }
}

fn tokenize(formula: &str) -> (Vec<Token>, Option<&'static str>) {
    let bytes = formula.as_bytes();
    let mut tokens = Vec::new();
    let mut position = 0usize;
    while position < bytes.len() {
        match bytes[position] {
            byte if byte.is_ascii_whitespace() => position += 1,
            b'+' => {
                tokens.push(Token::Plus);
                position += 1;
            }
            b'-' => {
                tokens.push(Token::Minus);
                position += 1;
            }
            b'*' => {
                tokens.push(Token::Multiply);
                position += 1;
            }
            b'/' => {
                tokens.push(Token::Divide);
                position += 1;
            }
            b'(' => {
                tokens.push(Token::Open);
                position += 1;
            }
            b')' => {
                tokens.push(Token::Close);
                position += 1;
            }
            byte if byte.is_ascii_digit() || byte == b'.' => {
                let start = position;
                position += 1;
                while position < bytes.len()
                    && (bytes[position].is_ascii_digit() || bytes[position] == b'.')
                {
                    position += 1;
                }
                let Some(raw) = formula.get(start..position) else {
                    return (tokens, Some("invalid_numeric_literal"));
                };
                let Ok(value) = Decimal::from_str(raw) else {
                    return (tokens, Some("invalid_numeric_literal"));
                };
                tokens.push(Token::Number(value));
            }
            byte if byte.is_ascii_alphabetic() || byte == b'$' => {
                let start = position;
                position += 1;
                while position < bytes.len()
                    && (bytes[position].is_ascii_alphanumeric() || bytes[position] == b'$')
                {
                    position += 1;
                }
                let Some(raw) = formula.get(start..position) else {
                    return (tokens, Some("invalid_cell_reference"));
                };
                let Some((row, column)) = parse_cell_ref(raw) else {
                    return (tokens, Some("unsupported_identifier"));
                };
                tokens.push(Token::Cell(row, column));
            }
            _ => return (tokens, Some("unsupported_formula_syntax")),
        }
    }
    (tokens, None)
}

fn parse_cell_ref(raw: &str) -> Option<(u32, u32)> {
    let normalized = raw.trim().replace('$', "").to_ascii_uppercase();
    let split = normalized.bytes().position(|byte| byte.is_ascii_digit())?;
    let (letters, digits) = normalized.split_at(split);
    if letters.is_empty()
        || letters.len() > 3
        || !letters.bytes().all(|byte| byte.is_ascii_uppercase())
        || digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let mut column = 0u32;
    for byte in letters.bytes() {
        column = column
            .checked_mul(26)?
            .checked_add(u32::from(byte - b'A' + 1))?;
    }
    let row = digits.parse::<u32>().ok()?;
    if !(1..=1_048_576).contains(&row) || !(1..=16_384).contains(&column) {
        return None;
    }
    Some((row - 1, column - 1))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use openconkit_spreadsheet::{
        DateSystem, IngestedCell, IngestedSheet, IngestedWorkbook, SheetKind, SheetVisibility,
        WorkbookFormat,
    };

    use super::*;

    fn sheet() -> IngestedSheet {
        let workbook = IngestedWorkbook {
            format: WorkbookFormat::Xlsx,
            date_system: DateSystem::Excel1900,
            sheets: vec![IngestedSheet {
                index: 0,
                name: "BOQ".into(),
                kind: SheetKind::Worksheet,
                visibility: SheetVisibility::Visible,
                declared_range: None,
                used_range: None,
                merged_regions: vec![],
                hidden_rows: None,
                hidden_columns: None,
                cells: vec![
                    numeric_cell(0, 0, "2"),
                    numeric_cell(0, 1, "3"),
                    numeric_cell(1, 0, "4"),
                ],
            }],
            total_cells: 3,
            total_text_bytes: 0,
        };
        workbook.sheets.into_iter().next().expect("sheet")
    }

    fn numeric_cell(row: u32, column: u32, value: &str) -> IngestedCell {
        IngestedCell {
            row,
            column,
            address: format!("{}{}", char::from(b'A' + column as u8), row + 1),
            raw_value: value.into(),
            normalized_value: NormalizedCellValue::Number(value.into()),
            displayed_value: None,
            formula: None,
        }
    }

    #[test]
    fn evaluates_arithmetic_cell_references_and_parentheses() {
        assert_eq!(
            evaluate(&sheet(), "=($A$1+B1)*2", (2, 2)),
            FormulaEvaluation::Value(Decimal::from(10))
        );
    }

    #[test]
    fn evaluates_one_rectangular_sum_range() {
        assert_eq!(
            evaluate(&sheet(), "SUM(A1:B2)", (2, 2)),
            FormulaEvaluation::Value(Decimal::from(9))
        );
    }

    #[test]
    fn unsupported_functions_and_division_by_zero_are_unverifiable() {
        assert!(matches!(
            evaluate(&sheet(), "ROUND(A1, 2)", (2, 2)),
            FormulaEvaluation::Unverifiable(_)
        ));
        assert_eq!(
            evaluate(&sheet(), "A1/0", (2, 2)),
            FormulaEvaluation::Unverifiable("division_by_zero")
        );
        assert_eq!(
            evaluate(&sheet(), "SUM(A1:B2)", (1, 1)),
            FormulaEvaluation::Unverifiable("self_reference")
        );
    }
}
