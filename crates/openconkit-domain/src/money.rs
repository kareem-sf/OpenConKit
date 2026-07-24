//! Money value objects.
//!
//! Canonical decimal representation: amounts are [`rust_decimal::Decimal`]
//! values serialized as JSON strings via its `serde-with-str` feature
//! (ADR 0008). Rationale: money must never go through binary floats, so no
//! rounding surprises cross the Rust/TypeScript boundary — the TS side sees
//! a canonical decimal string (`"1234.56"`) and can format or compare it
//! without float error.

use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::DomainError;

/// An ISO-4217-shaped currency code: exactly 3 uppercase ASCII letters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, TS)]
pub struct Currency(String);

impl Currency {
    /// Create a currency code, enforcing the ISO-4217 shape.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let valid = raw.len() == 3 && raw.bytes().all(|b| b.is_ascii_uppercase());
        if valid {
            Ok(Self(raw))
        } else {
            Err(DomainError::InvalidCurrency(raw))
        }
    }

    /// Borrow the code as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Currency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A monetary amount with an optional currency.
///
/// `currency: None` is the explicit, valid "unknown currency" state: source
/// workbooks frequently carry bare numbers, and inventing a currency would
/// be worse than admitting none was recorded (deliberate design decision
/// from the master spec). Consumers must handle `None` rather than assume
/// a default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MoneyAmount {
    /// The amount; serialized as a canonical decimal string (see module docs).
    #[ts(type = "string")]
    pub amount: Decimal,
    /// Currency of the amount, or `None` when unknown.
    pub currency: Option<Currency>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::str::FromStr;

    #[test]
    fn currency_accepts_three_uppercase_letters() {
        let sar = Currency::new("SAR").expect("valid code");
        assert_eq!(sar.as_str(), "SAR");
        assert_eq!(sar.to_string(), "SAR");
    }

    #[test]
    fn currency_rejects_invalid_codes() {
        for bad in ["", "SA", "SARR", "sar", "S1R", "S R"] {
            assert!(Currency::new(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn currency_deserialization_enforces_code_invariant() {
        let parsed: Result<Currency, _> = serde_json::from_str("\"sar\"");
        assert!(parsed.is_err());
    }

    #[test]
    fn money_amount_serializes_amount_as_json_string() {
        let money = MoneyAmount {
            amount: Decimal::from_str("1234.56").expect("valid decimal"),
            currency: Some(Currency::new("SAR").expect("valid code")),
        };
        let json = serde_json::to_value(&money).expect("serialize");
        assert_eq!(json["amount"], serde_json::json!("1234.56"));
        assert_eq!(json["currency"], serde_json::json!("SAR"));

        let back: MoneyAmount = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, money);
    }

    #[test]
    fn money_amount_unknown_currency_round_trips_as_null() {
        let money = MoneyAmount {
            amount: Decimal::new(-250, 2),
            currency: None,
        };
        let json = serde_json::to_string(&money).expect("serialize");
        assert!(json.contains("\"currency\":null"), "{json}");
        let back: MoneyAmount = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, money);
    }

    #[test]
    fn money_amount_maps_decimal_to_ts_string() {
        let cfg = ts_rs::Config::default();
        let decl = <MoneyAmount as TS>::decl(&cfg);
        assert!(decl.contains("amount: string"), "{decl}");
        assert!(decl.contains("currency: Currency | null"), "{decl}");
    }
}
