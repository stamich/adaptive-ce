# Analiza wyników benchmarków ACE 0.4.5-buildfix1

Źródło: `benchmark-0.4.5-buildfix1.zip` (21 rodzin, schema 2.0, Intel i7-9850H, 6C/12T, AVX2,
release). Dane surowe dołączone jako baseline w `examples/baselines/0.4.5-buildfix1/`.
Punkty odniesienia: przebieg 0.4.5 (ten sam sprzęt, poprzednie archiwum) oraz baseline
`0.4-buildfix2` używany przez bramki.

## 1. Bramki wydania (22 / 23 PASS)

| Bramka | Próg | 0.4.5 | 0.4.5-buildfix1 | Status |
|---|---:|---:|---:|---|
| planner recall / top-K | ≥ 0.99 / ≥ 0.98 | 1.0 / 1.0 | 1.0 / 1.0 | PASS |
| policy regret (mean / p95 / p99) | ≤ 16 / 64 / 256 B | 2.5 / 10 / 10 | 2.5 / 10 / 10 | PASS |
| BALANCED / DENSE ratio | ≥ 3.70× | 3.726 / 3.722 | 3.726 / 3.722 | PASS |
| FAST MB/s | ≥ 172.9 | **165.0 FAIL** | **205.3** | PASS (+24 %) |
| BALANCED MB/s | ≥ 93.6 | 114.3 | 124.7 | PASS |
| DENSE MB/s | ≥ 54.2 | 82.4 | 85.4 | PASS |
| **warm 64K range** | **≤ 71 683 ns** | 70 835 | **76 155** | **FAIL (+6.2 %)** |
| u64 timestamps MB/s | ≥ 90 | **89.2 FAIL** | **120.5** | PASS (+35 %) |
| delta-variable MB/s | ≥ 75 | 83.4 | 109.5 | PASS (+31 %) |
| numeric ratios, NumericFast FP | — | PASS | PASS | PASS |

Obie poprawki z buildfix1 zadziałały: FAST +24 %, NumericGeneral +25–35 %. Jedyną
niespełnioną bramką pozostał **warm64K**.

## 2. Przyczyna niespełnienia warm64K

`range_64k_warm` czyta 64 KiB z bloku **zerowego** (`rle+huffman`, 804 B fizycznie), ale musi
zdekodować cały blok 256 KiB. Rozkład czasu tego dekodowania (pomiar kontenerowy, mikrobenchmark):

| Etap | Czas | Udział |
|---|---:|---:|
| CRC32C całego bloku 256 KiB (`crc32c` crate, jeden łańcuch SSE4.2) | ~41–48 µs | **~55–60 %** |
| Huffman (4 096 symboli strumienia RLE) | ~20 µs | ~25 % |
| RLE → 256 KiB | ~11 µs | ~15 % |
| odczyt bloku + kopia 64 KiB | < 3 µs | — |

* Instrukcja `crc32` ma opóźnienie 3 cykli i przepustowość 1/cykl — jeden łańcuch wykorzystuje
  1/3 jednostki. To jest **stały koszt każdego dekodowania bloku** (także pełnej dekompresji
  i kodowania — CRC wejścia).
* Między 0.4.5 a buildfix1 ścieżka dekodowania nie była zmieniana, a te same oznaki (−10,6 %
  lz4, −22 % dekompresji DENSE, `threads-4` cv 21 %) pokazują ~5–10 % szumu maszyny
  w tym przebiegu. Bramka była więc na granicy marginesu już w 0.4.5 (70,8 vs 71,7 µs).

Wniosek: bramkę należało zamknąć strukturalnie (CRC, Huffman, kopie), nie strojeniem.

## 3. Pozostałe obserwacje według rodzin

* **compression / corpus / block-matrix / block-policy / streaming:** +8…+11 % kodowania (delta-series +40 %)
  względem 0.4.5 przy identycznych ratio (deterministyczność potwierdzona). Auto block policy
  wybiera 1 MiB dla `balanced`/`sequential` (mixed: 149 vs 123 MB/s, ratio 3.732) i 256 KiB dla
  `random-access` — zgodnie z koncepcją §37–38.
* **numeric-ablation:** `planner-v4` dla u64 timestamps 123 MB/s vs bezpośredni kodek 239 MB/s;
  narzut plannera (~50 %) to głównie analiza generyczna — kandydat do 0.4.1 (pomijanie
  `analyze_with_level` gdy NumericMargin jest dominujący).
* **numeric-general / gauge-sawtooth:** Numeric 6,29 MB vs plan generyczny 0,86 MB — planner
  poprawnie wybiera LZ (piła wraca co 4096 wartości, delta-of-delta nie pomaga). OK.
* **monotonic-outliers:** 1,78× bez zmian — wymaga Patched FOR (backlog 0.4.1).
* **parallel:** skalowanie 1→8 wątków 4,6× (124→567 MB/s); 12 wątków nie daje zysku
  (6 rdzeni fizycznych, SMT). `threads-4` ma cv 21 % — wynik niewiarygodny w tym przebiegu.
* **policy-oracle-v2 / planner:** regret globalny 732 B w bloku zerowym wynika z
  `RunLengthDominance` (świadoma strata 252 B za szybsze dekodowanie RLE) — diagnostyka,
  nie błąd.
* **entropy:** rANS4x +3,9 % ratio vs Huffman na `mixed`; czasu ta rodzina nie mierzy.
* **random-access / decode_block:** blok numeryczny (id 20) dekoduje się najwolniej
  (~410 µs) — generyczny dekoder NUM1 alokował 4 wektory pośrednie na blok.

## 4. Działania w 0.4.5-buildfix2 (wynikające z analizy)

| Problem | Zmiana | Efekt (kontener, A/B ten sam binarny harness) |
|---|---|---|
| CRC32C = 55–60 % dekodowania bloku | `ace_simd::crc32c_hardware`: 3 niezależne łańcuchy SSE4.2 + łączenie w GF(2) | CRC 256 KiB: 41 → 12 µs |
| Huffman bit-po-bicie | tablica 11-bitowa + 64-bitowy bufor czytnika, kanoniczny fallback | 4 096 sym.: 20 → 15,6 µs; 256 KiB JSON: 2 246 → 1 043 µs |
| NUM1 decode: 4 alokacje pośrednie | fuzja `unpack_iter → unzigzag → undelta_iter → write_lanes_from_iter` | delta-var 529 → 327 µs, losowe <5000: 426 → 215 µs |
| RAW: podwójna kopia | `decode_entropy_cow` (Cow dla `None`) | blok RAW 72 → 32 µs (razem z CRC) |
| Estymator NUM1: max po wartościach | akumulacja OR bitów (ten sam najwyższy bit) | estimate 2× szybciej |

Wynik bramek w kontenerze po zmianach: **23 / 23 PASS**, warm64K **40,0 µs** (próg 71,7 µs).

### A/B w kontenerze (0.4.5-buildfix1 vs 0.4.5-buildfix2, ten sam sprzęt, mediana z 7)

Wartości bezwzględne kontenera różnią się od i7-9850H; istotne są proporcje. Ratio identyczne
we wszystkich wierszach (wyjście bajt-w-bajt identyczne — patrz §5).

| Workload | Metric | 0.4.5-buildfix1 | 0.4.5-buildfix2 | Δ |
|---|---|---:|---:|---:|
| compression / mixed_16m / ace-fast | enc | 168 | 181 | +8 % |
| compression / mixed_16m / ace-fast | dec | 1,306 | 1,861 | +43 % |
| compression / mixed_16m / ace-balanced | enc | 108 | 116 | +7 % |
| compression / mixed_16m / ace-balanced | dec | 1,362 | 1,842 | +35 % |
| compression / mixed_16m / ace-dense | enc | 79 | 87 | +11 % |
| compression / mixed_16m / ace-dense | dec | 1,345 | 1,535 | +14 % |
| numeric / u32-counter / planner_v4_3 | enc | 2,325 | 3,726 | +60 % |
| numeric / u32-counter / planner_v4_3 | dec | 3,118 | 5,064 | +62 % |
| numeric / u64-timestamps / planner_v4_3 | enc | 125 | 171 | +37 % |
| numeric / u64-timestamps / planner_v4_3 | dec | 985 | 1,130 | +15 % |
| numeric / gauge-sawtooth / planner_v4_3 | enc | 63 | 69 | +11 % |
| numeric / gauge-sawtooth / planner_v4_3 | dec | 761 | 1,126 | +48 % |
| numeric / monotonic-outliers / planner_v4_3 | enc | 102 | 117 | +15 % |
| numeric / monotonic-outliers / planner_v4_3 | dec | 384 | 598 | +56 % |
| numeric / delta-variable / planner_v4_3 | enc | 113 | 143 | +27 % |
| numeric / delta-variable / planner_v4_3 | dec | 543 | 622 | +15 % |
| random-access / mixed_16m / full_decompress | time-based MB/s | 1,327 | 1,720 | +30 % |
| random-access / mixed_16m / decode_block / block 0 | time-based MB/s | 3,655 | 6,473 | +77 % |
| random-access / mixed_16m / decode_block / block 20 | time-based MB/s | 713 | 838 | +18 % |
| random-access / mixed_16m / decode_block / block 40 | time-based MB/s | 2,250 | 4,355 | +94 % |
| random-access / mixed_16m / decode_block / block 60 | time-based MB/s | 3,130 | 8,223 | +163 % |
| random-access / mixed_16m / range_64k_cold | time-based MB/s | 826 | 1,524 | +84 % |
| random-access / mixed_16m / range_64k_warm | time-based MB/s | 778 | 1,563 | +101 % |
| random-access / mixed_16m / range_cross_2 | time-based MB/s | 769 | 1,562 | +103 % |
| random-access / mixed_16m / range_cross_4 | time-based MB/s | 2,104 | 4,186 | +99 % |
| corpus / zeros_16m / ace-balanced | enc | 115 | 127 | +11 % |
| corpus / zeros_16m / ace-balanced | dec | 2,280 | 3,728 | +64 % |
| corpus / low-cardinality_16m / ace-balanced | enc | 63 | 73 | +14 % |
| corpus / low-cardinality_16m / ace-balanced | dec | 436 | 605 | +39 % |
| corpus / runs_16m / ace-balanced | enc | 113 | 123 | +9 % |
| corpus / runs_16m / ace-balanced | dec | 1,999 | 3,441 | +72 % |
| corpus / numeric-u32_16m / ace-balanced | enc | 2,541 | 3,854 | +52 % |
| corpus / numeric-u32_16m / ace-balanced | dec | 3,167 | 4,751 | +50 % |
| corpus / delta-series_16m / ace-balanced | enc | 117 | 149 | +27 % |
| corpus / delta-series_16m / ace-balanced | dec | 979 | 1,151 | +18 % |
| corpus / structured-json_16m / ace-balanced | enc | 67 | 71 | +6 % |
| corpus / structured-json_16m / ace-balanced | dec | 1,779 | 2,747 | +54 % |
| corpus / random_16m / ace-balanced | enc | 172 | 201 | +17 % |
| corpus / random_16m / ace-balanced | dec | 2,173 | 3,973 | +83 % |
| corpus / mixed_16m / ace-balanced | enc | 104 | 121 | +16 % |
| corpus / mixed_16m / ace-balanced | dec | 1,260 | 1,786 | +42 % |
| streaming / mixed_16m / bounded_stream | time-based MB/s | 102 | 109 | +7 % |
| streaming / mixed_64m / bounded_stream | time-based MB/s | 100 | 111 | +11 % |
| parallel / mixed_16m / threads-1 | enc | 107 | 120 | +12 % |
| parallel / mixed_16m / threads-2 | enc | 201 | 225 | +12 % |

## 5. Weryfikacja

* Wyjście `ace compress` (FAST/BALANCED/DENSE) i `compress-stream` jest **bajt-w-bajt identyczne**
  z 0.4.5-buildfix1 dla całego Corpus V3 (15 plików × 3 profile) oraz danych u16/ns.
* Dekoder 0.4.5-buildfix1 poprawnie czyta pliki 0.4.5-buildfix2 (format bez zmian).
* `cargo test --workspace`: 184 testy PASS; `cargo clippy --workspace --all-targets`: 0 ostrzeżeń
  (MSRV 1.75 z `clippy.toml`); wszystkie cele fuzz kompilują się.

## 6. Rekomendacje na 0.4.1

1. Pominięcie analizy generycznej dla bloków NumericGeneral z dominującym NumericMargin
   (szacunkowo +30–50 % kodowania u64 timestamps).
2. Patched FOR dla `monotonic-outliers`.
3. Bramka FAST świadoma ratio (MB/s przy stałym lub lepszym ratio), bo FAST 0.4.5 kompresuje
   3,68× zamiast 1,98× buildfix2.
4. Powtórzenie `threads-4` / `range_*` z większą liczbą prób (cv > 10 %).
