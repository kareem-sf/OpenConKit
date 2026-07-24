# 0008. Canonical decimal semantics for money and quantities

- Status: Accepted
- Date: 2026-07-22

## Context

BOQ analysis compares amounts (quantities x unit prices vs. stated totals).
Binary floating point (`f64`) cannot represent decimal currency values
exactly (`0.1 + 0.2 != 0.3`), which would make arithmetic findings
nondeterministic and tolerance checks unreliable across platforms.

## Decision

- All money/quantity values in the domain use `rust_decimal::Decimal`
  (96-bit decimal), never `f64`.
- The canonical serialized representation is a **decimal string**:
  `rust_decimal` is built with the `serde-with-str` feature, so JSON and
  SQLite text columns carry values like `"1234.50"` with no float
  round-trip.
- TypeScript contracts type these fields as `string` (`#[ts(type = "string")]`),
  matching the wire format; parsing on the frontend happens only at display
  boundaries.
- `MoneyAmount.currency` is `Option<Currency>`; `None` is the explicit,
  valid "unknown currency" state (source workbooks often omit currency).
  Unknown currency never blocks analysis; it only suppresses
  currency-specific formatting.
- Tolerances in settings (`absolute_tolerance`, `relative_tolerance`) are
  decimals as well, so amount checks are exact decimal arithmetic.

## Consequences

- Positive: deterministic, platform-independent amount comparisons;
  no rounding drift between Rust, SQLite, JSON and TypeScript.
- Negative: `Decimal` is slower than `f64` (irrelevant at BOQ scale) and
  the frontend must parse strings for display math.
- Alternatives considered: `f64` with epsilon comparisons (rejected:
  nondeterministic at scale); integer minor units (rejected: BOQ
  quantities are not necessarily currency minor units and carry varying
  precision); `bigdecimal` (rejected: heavier, unbounded precision not
  needed).
