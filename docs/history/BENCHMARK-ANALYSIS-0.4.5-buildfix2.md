# Analiza wyników benchmarków ACE 0.4.5-buildfix2

Źródło: `benchmark-0.4.5-buildfix2.zip` (21 rodzin, schema 2.0). Porównanie z przebiegiem
0.4.5-buildfix1 (`examples/baselines/0.4.5-buildfix1/`). Ten sam sprzęt w obu przebiegach:
Intel i7-9850H (6C/12T, AVX2), 32 GB RAM, build release.

## 1. Podsumowanie

* **23 / 23 bramek PASS.** Jedyna bramka niespełniona w buildfix1 (warm64K) przechodzi teraz
  z zapasem **2,26×**: 76,2 → **31,7 µs** (próg 71,7 µs).
* **Wszystkie współczynniki kompresji są identyczne** z buildfix1 we wszystkich rodzinach
  (Corpus, block-matrix, block-policy, numeric, entropy, planner, policy oracle). To
  potwierdza na maszynie referencyjnej, że refaktoryzacja nie zmieniła ani jednej decyzji
  plannera ani jednego zakodowanego bajtu. Liczniki plannera (752 estymowanych, 272
  próbkowanych kandydatów, 0 pełnych prób) też są identyczne.
* **Dekodowanie +36…+134 %, losowy dostęp −52…−68 % czasu, kodowanie numeryczne
  +33…+74 %.** Te zyski są wielokrotnie większe niż szum pomiaru.
* **Kodowanie generyczne +7…+13 %.** Mniej więcej połowa tego wzrostu to prawdopodobnie zmiana
  warunków maszyny (lz4 w tym samym przebiegu +10,6 %), więc nie należy go przypisywać
  w całości kodowi.
* Po raz pierwszy **ACE dekoduje `mixed_16m` szybciej niż zstd-3** (BALANCED 2 422 vs
  2 280 MB/s) przy praktycznie tym samym ratio (3,726 vs 3,760).

## 2. Wiarygodność pomiaru

Kodeki zewnętrzne nie zmieniły się między przebiegami, więc ich wahania pokazują szum maszyny:

| Kodek zewnętrzny | kodowanie | dekodowanie |
|---|---:|---:|
| lz4 | +10,6 % | −2,8 % |
| zstd-3 | −5,0 % | −8,9 % |
| gzip-6 | −2,9 % | −1,0 % |

Pasmo szumu to więc około **±10 %**. Zmiany ACE w tym paśmie (kodowanie generyczne
+7…+13 %, streaming +10 %, stability +9,5 %) są „prawdopodobne, ale niepotwierdzone”. Zmiany
poza pasmem są rzeczywiste: dekodowanie, losowy dostęp i ścieżka numeryczna.

Uwaga: rozrzut dekodowania (cv) wzrósł do 21–51 % (wcześniej 11–27 %). Przy obecnych
czasach (3–7 ms na 16 MiB) siedem prób to za mało, a dominować zaczynają koszty stałe
(alokacja i zerowanie bufora 16 MiB, page faulty). Mediany i minima są spójne
(min −44…−47 %), więc kierunek jest pewny, ale pojedyncze procenty dekodowania
należy traktować ostrożnie.

## 3. Bramki z marginesem

| Bramka | Próg | buildfix1 | buildfix2 | Zapas |
|---|---:|---:|---:|---:|
| warm64K | ≤ 71 683 ns | 76 155 ✗ | **31 710** | 2,26× |
| FAST MB/s | ≥ 172,9 | 205,3 | **224,5** | 1,30× |
| BALANCED MB/s | ≥ 93,6 | 124,7 | **135,2** | 1,44× |
| DENSE MB/s | ≥ 54,2 | 85,4 | **93,7** | 1,73× |
| u64 timestamps MB/s | ≥ 90 | 120,5 | **179,6** | 2,00× |
| delta-variable MB/s | ≥ 75 | 109,5 | **158,8** | 2,12× |
| u32 NumericFast MB/s | ≥ 343 | 2 858 | **4 442** | 12,9× |
| BALANCED / DENSE ratio | ≥ 3,70 | 3,726 / 3,722 | 3,726 / 3,722 | 1,007× |
| policy regret mean / p95 / p99 | 16 / 64 / 256 B | 2,5 / 10 / 10 | 2,5 / 10 / 10 | ≥ 6× |

Najmniejszy zapas mają bramki ratio (0,7 %), ale są deterministyczne i nie zależą od szumu.
Zmieni je dopiero inna decyzja plannera, nie zmiana wydajności.

## 4. Zmiany według rodzin

### 4.1 Kompresja `mixed_16m` i porównanie z innymi kodekami

| Kodek | Ratio | Kodowanie MB/s | Dekodowanie MB/s |
|---|---:|---:|---:|
| ACE FAST | 3,682 | 205 → **225** | 1 360 → **2 571** (+89 %) |
| ACE BALANCED | 3,726 | 125 → **135** | 1 353 → **2 422** (+79 %) |
| ACE DENSE | 3,722 | 85 → **94** | 1 090 → **2 275** (+109 %) |
| zstd-3 | 3,760 | 1 214 | 2 280 |
| lz4 | 3,170 | 1 907 | 2 706 |
| gzip-6 | 3,636 | 68 | 663 |

* ACE ma ratio zstd-3 (−0,9 %) i dekoduje szybciej od niego. Kodowanie jest ~9× wolniejsze,
  bo ACE analizuje i planuje każdy blok (patrz §5).
* W porównaniu z gzip-6 ACE BALANCED ma wyższe ratio, 2× szybsze kodowanie i 3,7× szybsze
  dekodowanie.

### 4.2 Dekodowanie Corpus V3 (BALANCED)

| Plik | Dekodowanie MB/s | Zmiana |
|---|---:|---:|
| runs | 1 989 → 4 112 | +107 % |
| structured-json | 1 903 → 3 466 | +82 % |
| numeric-u32 | 3 094 → 5 283 | +71 % |
| mixed | 1 363 → 2 259 | +66 % |
| zeros | 2 725 → 4 475 | +64 % |
| random | 2 228 → 3 629 | +63 % |
| low-cardinality | 482 → 692 | +44 % |
| delta-series | 930 → 1 336 | +44 % |

Główne źródło wzrostu to CRC32C (stały koszt każdego bloku). RAW (`random`) i RLE (`runs`,
`zeros`) zyskują najwięcej, bo poza CRC robią niewiele. `low-cardinality` ma najwolniejsze
dekodowanie (692 MB/s). Prawdopodobnie dominuje tam rANS (stan skalarny, bajt po bajcie).
To kandydat na kolejną optymalizację.

### 4.3 Losowy dostęp

| Operacja | buildfix1 | buildfix2 | Zmiana |
|---|---:|---:|---:|
| range 64K warm / cold | 76,2 / 79,0 µs | **31,7 / 31,7 µs** | −58 / −60 % |
| range cross 2 / 4 bloki | 158 / 434 µs | **64 / 141 µs** | −60 / −68 % |
| decode_block zeros / json / random | 75 / 114 / 76 µs | **30 / 47 / 34 µs** | −60 / −59 / −56 % |
| decode_block numeric | 414 µs | **202 µs** | −51 % |
| full decompress 16 MiB | 13,6 ms | **7,3 ms** | −46 % |
| decoder_open | 1,15 µs | 0,81 µs | −30 % |

Odczyt 4 KiB kosztuje teraz tyle samo co 64 KiB (~30 µs). Czas wyznacza dekodowanie całego
bloku 256 KiB, a CRC to już tylko około 40 % tego czasu (~12 µs). Przy profilu `RandomAccess`
pozostaje to głównym kosztem małych odczytów (patrz §6, pkt 4).

### 4.4 Ścieżka numeryczna

| Workload | Kodowanie MB/s | Dekodowanie MB/s |
|---|---:|---:|
| u32-counter (NumericFast) | 2 858 → **4 442** (+55 %) | 2 983 → **5 266** (+77 %) |
| u64-timestamps | 120 → **180** (+49 %) | 971 → **1 325** (+36 %) |
| delta-variable | 110 → **159** (+45 %) | 517 → **875** (+69 %) |
| monotonic-outliers | 100 → **134** (+34 %) | 361 → **845** (+134 %) |
| gauge-sawtooth | 69 → **77** (+11 %) | 1 105 → **1 552** (+40 %) |

Ablacja (u64 timestamps):

| Ścieżka | buildfix1 MB/s | buildfix2 MB/s | Zmiana |
|---|---:|---:|---:|
| bezpośredni kodek NUM1 | 227 | 510 | +124 % |
| Planner V4 | 123 | 183 | +48 % |

Kodek przyspieszył bardziej niż planner. **Narzut plannera wzrósł więc z ~46 % do ~64 %
czasu kodowania** u64 timestamps i jest teraz głównym ogranicznikiem ścieżki numerycznej.
`gauge-sawtooth` zyskał najmniej, bo planner wybiera tam ścieżkę generyczną (LZ), a dokładny
szacunek NUM1 jest liczony i odrzucany.

### 4.5 Polityka rozmiaru bloku

| Wariant | buildfix1 MB/s | buildfix2 MB/s | Zmiana |
|---|---:|---:|---:|
| mixed auto-balanced / auto-sequential (1 MiB) | 149 | **167** | +11 % |
| mixed auto-random-access (256 KiB) | 125 | **134** | +7 % |
| numeric (każdy wariant) | 2 789–2 999 | **4 433–4 940** | +59…+66 % |

Ratio się nie zmieniło: mixed 3,732 / 3,726, numeric do 9 001×. Wybór rozmiaru przez Auto
jest identyczny jak w buildfix1.

### 4.6 Skalowanie wątków (`mixed_16m`, kodowanie)

| Wątki | buildfix1 MB/s (efekt.) | buildfix2 MB/s (efekt.) |
|---:|---:|---:|
| 1 | 124 (1,00) | 137 (1,00) |
| 2 | 236 (0,96) | 260 (0,95) |
| 4 | 326 (0,66) | **434 (0,79)** |
| 6 | 434 (0,58) | 453 (0,55) |
| 8 | 567 (0,57) | 558 (0,51) |
| 12 | 549 (0,37) | 595 (0,36) |

* Wynik dla 4 wątków w buildfix1 był zaburzony (cv 21 %); teraz jest stabilny.
* Przy 6–8 wątkach efektywność spadła o 5–11 %. Praca na blok się skróciła, a część
  sekwencyjna (składanie kontenera w kolejności, alokacje, zapis indeksu) nie, więc zgodnie
  z prawem Amdahla rośnie jej udział. 6 rdzeni fizycznych ogranicza skalowanie powyżej
  6 wątków.

### 4.7 Streaming, pamięć, planner

* Streaming: 115 → 127 MB/s (16 MiB) i 114 → 126 MB/s (64 MiB). Bufor źródła nadal
  ograniczony do jednego bloku (262 144 B), mimo przejścia na wspólny `AceWriter`.
* Memory: scratch 787 456 B, pojemność zachowana po resecie (bez zmian).
* Planner, policy-oracle-v2, planner-route: wszystkie decyzje, recall, regret i trasy
  identyczne. Regret globalny 185,5 B/blok pozostaje diagnostyczny (świadome dominacje RLE).
* Hot path: `mixed_generic_analysis_ns` = 55,7 ms (buildfix1: 54,0 ms, w paśmie szumu);
  klasyfikacja trasy ~0,37 ms.

## 5. Gdzie teraz idzie czas

| Ścieżka | Główny koszt | Udział |
|---|---|---|
| Kodowanie BALANCED `mixed_16m` (118 ms) | analiza generyczna bloków (`DefaultBlockAnalyzer`) | **~47 %** (55,7 ms) |
| Kodowanie u64 timestamps | planner (analiza + ewaluacja) wokół kodeka NUM1 | **~64 %** |
| Odczyt losowy 4–64 KiB | dekodowanie całego bloku 256 KiB (Huffman/RLE + CRC) | ~100 % |
| Dekodowanie `low-cardinality` | rANS skalarny | najwolniejszy plik Corpus |

## 6. Rekomendacje (kolejność według zysku i ryzyka)

1. **Pominięcie analizy generycznej, gdy NumericMargin dominuje** (NumericGeneral, dokładny
   rozmiar NUM1 ≪ szacunek generyczny). Szacunkowo +40–60 % kodowania u64 timestamps
   i delta-variable bez zmiany ratio na Corpus V3.
2. **Przyspieszenie `DefaultBlockAnalyzer`.** Histogram H0/H1 i hashowanie powtórzeń to ~47 %
   kodowania generycznego. Kandydaci: histogram z 4 tablicami (bez zależności zapisu),
   próbkowanie H1 dla FAST, AVX2 dla run score. Cel: BALANCED > 160 MB/s.
3. **Wiarygodność benchmarku dekodowania:** więcej prób (np. 15) albo minimum zamiast mediany
   dla przebiegów < 10 ms, wstępnie zaalokowany bufor wyjścia. Obecny cv 21–51 % utrudni
   wykrywanie przyszłych regresji ±10 %.
4. **Odczyt losowy małych zakresów:** opcjonalny cache ostatnio zdekodowanych bloków w
   `AceIndexedDecoder` (bez zmiany formatu). Kolejne odczyty z tego samego bloku byłyby wtedy
   prawie darmowe. Benchmark musiałby wtedy osobno mierzyć trafienie i chybienie cache'u,
   bo obecny `range_64k_warm` czyta ciągle ten sam blok.
5. **rANS:** interleaving 2×/4× stanów w dekoderze (format rANS4x już to umożliwia),
   z myślą o `low-cardinality`.
6. Utrzymane z backlogu 0.4.1: Patched FOR (`monotonic-outliers` 1,78×) oraz bramka FAST
   świadoma ratio.

## 7. Werdykt

0.4.5-buildfix2 spełnia wszystkie bramki z dużym zapasem na maszynie referencyjnej
i zachowuje bajtową zgodność wyjścia. Nie widzę regresji wykraczającej poza szum. Jedyny
spadek, efektywność przy 6–8 wątkach, wynika z szybszej pracy na blok, nie z wolniejszego
kodu. Wersja nadaje się na zamknięcie linii 0.4.5. Dalsze zyski są po stronie plannera
i analizatora (pkt 1–2), nie kodeków.
