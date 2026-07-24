# BOQ Inspector

BOQ Inspector is OpenConKit's deterministic Bill of Quantities quality-review
tool. It discovers candidate BOQ tables in nonstandard Excel workbooks,
normalizes commercial fields without changing the workbook, and produces
traceable findings with sheet/cell evidence.

## Inputs and source safety

- Accepts `.xls` and `.xlsx` workbooks up to the limit declared by the tool
  manifest.
- Imports by bounded, read-only streaming copy into the project's immutable
  source vault and records SHA-256 provenance.
- Never writes to the selected source workbook. Every analysis run and report
  is a new managed artifact under the OpenConKit application home.
- Applies archive-expansion, worksheet-dimension, cell-count, and parser-work
  limits. Unsupported or ambiguous behavior is reported as uncertain or
  unverifiable instead of guessed.

## Structure detection

The engine inventories visible and hidden sheets, evaluates used ranges and
density, segments multiple candidate tables, detects bilingual or headerless
layouts, assigns column roles with confidence, classifies rows, and retains
stable source references. It recognizes item number, description, unit,
quantity, rate, amount, currency, notes, and unknown roles.

Arabic and English header aliases, Arabic-Indic and Western digits, common
decimal/grouping conventions, currencies, and BOQ unit aliases are normalized
for comparison. Original text and exact source evidence remain authoritative;
unit normalization never performs measurement conversion.

## Deterministic checks

The shipped rule set covers:

- missing description, unit, quantity, rate, amount, and required columns;
- zero and negative commercial values;
- amount versus quantity multiplied by rate using configured precision and
  absolute/relative tolerances;
- spreadsheet error cells, broken formula results, supported formula-result
  mismatches, and explicitly unverifiable formulas;
- exact and cross-sheet duplicates, deterministic fuzzy-description
  candidates, inconsistent units, and cross-sheet inconsistencies;
- subtotal and total mismatches when the range can be verified;
- robust within-context value outliers labeled for professional review;
- low-confidence structures, ambiguous/unmapped columns, and Pareto
  concentration summaries where amounts are comparable.

Findings are suggestions for professional review, not edits. Their rule id,
rule-set version, severity, category, confidence, original value/formula, and
evidence references are persisted with the run.

## Formula boundary

OpenConKit is not an Excel recalculation engine. Formula verification is
limited to a documented safe same-sheet arithmetic subset and `SUM` ranges
over numeric cells. External links, macros, volatile functions, dynamic
arrays, and unsupported syntax are never executed. They are marked
unverifiable for manual review.

## Results and history

Completed outputs are stored atomically with the run and authoritative
findings. The UI can reopen a historical run, search/filter/sort findings,
inspect evidence, review detected structure and confidence, and reproduce
reports from the exact stored output. History also records the source hash,
app/tool/rule-set versions, report count, and optional AI status.

## Reports

The tool provides independent English and Arabic:

- Excel (`.xlsx`) reports with metadata, summary, findings, detection,
  evidence, Pareto output, and limitations.
- PDF reports rendered in-process with Typst, fixed trusted templates,
  embedded fonts, Arabic shaping, RTL layout, evidence, and limitations.

Report cells are written as data, not workbook formulas. Generated artifacts
are unique and never overwrite previous reports. Before revealing a recorded
report in the operating-system file manager, the desktop host revalidates path
confinement, rejects symbolic links, and verifies the persisted SHA-256.
AI commentary is not placed in a report unless the optional AI workflow has
produced a separately stored, schema-valid result; the current AI-disabled
workflow therefore emits deterministic reports only.

## Optional AI

Deterministic analysis and exports require no account or network. AI review is
an optional, separately stored commentary layer. When the Codex integration is
enabled, only the selected run's extracted facts and stable evidence ids may be
sent after informed consent. AI references must validate against the
authoritative run and can never become deterministic findings or modify the
source workbook.

## Testing

Run the focused gates with:

```text
cargo test -p openconkit-tool-boq-inspector --all-features
pnpm --filter @openconkit/desktop-ui test
pnpm tool:completeness
```

Engine unit tests cover normalization, formula boundaries, structure
detection, required rules, cancellation, schemas, and both report providers.
Repository-level integration tests cover immutable source hashing, atomic
run/output persistence, report reproducibility, and export-path integrity.
