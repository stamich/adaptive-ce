# Policy Oracle V2

## Why a third oracle

Global oracle answers "what is smallest?".
Route oracle answers "what is allowed?".
Policy oracle answers "what should ACE choose?".

The distinction matters for zero/run-heavy blocks where Numeric may be globally smaller but RLE
offers materially cheaper decode/random-access behavior for a small accepted size premium.

## Preference

Preference classes:

- Preferred
- Neutral
- Penalized
- DiagnosticOnly
- Rejected

`DominanceEnvelope` limits how much size may be sacrificed for a preferred decode behavior.

## Release semantics

Policy oracle drives:

- candidate-generation recall;
- Top-K recall;
- mean regret;
- p95/p99 regret.

Global and route regret remain visible but diagnostic-only.
