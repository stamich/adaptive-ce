# ACE 0.4-buildfix4 — implementation order

## Phase A — freeze baselines and measure regressions
1. Use 0.4-buildfix3-buildfix1 as functional base.
2. Preserve the exact 0.4-buildfix2 performance baseline.
3. Preserve Format 1.3 bytes and reader compatibility 1.0–1.3.
4. Preserve NumericGeneral candidate cap <=5.
5. Preserve NumericGeneral verification cap <=2.
6. Preserve exact Numeric no-sample behavior.
7. Preserve NumericGeneral second-stage disable.
8. Preserve zero full trial encodes.
9. Add fine-grained route/generic-analysis timing.
10. Add planner-hotpath benchmark.
11. Add random-access-plan-diff benchmark.

## Phase B — strict NumericFast evidence
12. Add `NumericFastEvidence`.
13. Make full-block validator return evidence instead of only bool.
14. Store width in evidence.
15. Store first value in evidence.
16. Store constant first delta in evidence.
17. Store value count in evidence.
18. Store tail byte count in evidence.
19. Require non-zero first delta.
20. Require complete-block monotonicity.
21. Require exactly constant first delta across the complete block.
22. Tighten prefilter monotonic threshold to >=0.999.
23. Tighten constant-delta threshold to >=0.999.
24. Tighten non-zero delta threshold to >=0.999.
25. Keep a compatibility bool validator backed by evidence.
26. Add outlier false-fast test.
27. Add sawtooth false-fast test.
28. Keep fixed u32 counter as NumericFast.
29. Keep fixed u64 timestamps as NumericFast.

## Phase C — single PlanningContext
30. Add `planning_context.rs`.
31. Add `PlanningContext`.
32. Classify one block once.
33. Record route-classification time.
34. Carry one authoritative `RouteDecision`.
35. Carry NumericFast evidence inside RouteDecision.
36. Add route-reusing evaluator entry point.
37. Keep compatibility evaluator wrapper for external/tests.
38. Make production engine use PlanningContext.
39. Make production engine never reclassify NumericFast.
40. Make production engine never revalidate NumericFast.
41. Make explain reuse the same route decision.

## Phase D — direct NumericFast production encode
42. Add `numeric_encode_fixed_step`.
43. Reuse NUM1 header.
44. Write DeltaOfDelta mode directly.
45. Write bit_width=0 directly.
46. Reuse first value from evidence.
47. Reuse first delta from evidence.
48. Avoid `estimate_numeric`.
49. Avoid FOR/Delta/DoD mode search.
50. Avoid delta/DoD temporary vectors.
51. Avoid bit-width scan.
52. Avoid bit packing.
53. Preserve tail bytes.
54. Validate evidence/input length.
55. Add direct fixed-step codec roundtrip test.
56. Use direct encoder only for validated NumericFast.
57. Preserve generic Numeric codec for NumericGeneral.

## Phase E — CandidatePreference / dominance policy
58. Add `dominance_policy.rs`.
59. Add `CandidatePreference`.
60. Add Preferred.
61. Add Neutral.
62. Add Penalized.
63. Keep DiagnosticOnly.
64. Keep Rejected.
65. Add `DominanceReason`.
66. Add RunLengthDominance.
67. Add IncompressibleRawDominance.
68. Add NumericDominance.
69. Add RandomAccessPreference.
70. Add `DominanceEnvelope`.
71. FAST envelope: 4096 B / 2%.
72. BALANCED envelope: 2048 B / 1%.
73. DENSE envelope: 512 B / 0.5%.
74. Expand envelope for RandomAccess.
75. Prefer RLE for zero/run-heavy data.
76. Prefer RAW for incompressible data.
77. Prefer Numeric for NumericFast/NumericGeneral.
78. Penalize expensive decode families for RandomAccess where appropriate.
79. Add Rustdoc to all new public policy APIs.
80. Add dominance-policy tests.

## Phase F — PolicyOracle V2
81. Add `policy_oracle.rs`.
82. Add `PolicyOracleCandidate`.
83. Add `PolicyOracleDecision`.
84. Filter through RoutePolicy/DominancePolicy.
85. Keep global oracle diagnostic.
86. Keep route oracle diagnostic.
87. Add policy oracle as release truth.
88. Apply preference tier first.
89. Apply size envelope inside preference tier.
90. Fall back to smallest route-eligible candidate.
91. Report preference class.
92. Report dominance reason.
93. Report accepted size loss.
94. Keep PolicyOracle outside production hot path.

## Phase G — policy quality benchmark
95. Rename oracle benchmark to `policy-oracle-v2`.
96. Report global oracle.
97. Report route oracle.
98. Report policy oracle.
99. Report global regret.
100. Report route regret.
101. Report policy regret.
102. Report preference.
103. Report dominance reason.
104. Report accepted size loss.
105. Use policy oracle for candidate recall.
106. Use policy oracle for Top-K recall.
107. Use policy oracle for mean regret.
108. Use policy oracle for p95/p99 regret.

## Phase H — hot-path/random-access benchmarks
109. Add `planner-hotpath`.
110. Cover mixed.
111. Cover random.
112. Cover structured.
113. Cover zeros.
114. Cover u32 counter.
115. Cover u64 timestamps.
116. Report route classify time.
117. Report generic analysis time.
118. Report planning time.
119. Report production encode time.
120. Report wall-clock throughput.
121. Add `random-access-plan-diff`.
122. Report current plan distribution.
123. Point to bundled buildfix2 baseline.
124. Measure warm 4K.
125. Measure warm 16K.
126. Measure warm 64K.

## Phase I — release gates
127. Policy candidate recall >=0.99.
128. Policy Top-K recall >=0.98.
129. Policy mean regret <=16 B/block.
130. Policy p95 <=64 B.
131. Policy p99 <=256 B.
132. Full trials ==0.
133. BALANCED ratio >=3.70x.
134. DENSE ratio >=3.70x.
135. FAST throughput >=95% buildfix2.
136. BALANCED throughput >=95% buildfix2.
137. DENSE throughput >=95% buildfix2.
138. Warm64K <=110% buildfix2.
139. Determinism true.
140. u32 NumericFast >=95% buildfix2.
141. u64 timestamps >=90 MB/s.
142. delta-variable >=75 MB/s.
143. NumericFast false-positive rate ==0 on negative corpus.

## Phase J — naming/cleanup
144. Every shell script must contain `0.4-buildfix4`.
145. Every Python script must contain `0.4-buildfix4`.
146. Rename benchmark runner.
147. Rename comparison runner.
148. Rename demo runner.
149. Rename build runner.
150. Rename all tool Python scripts.
151. Prefix every benchmark-result/baseline JSON with `benchmark-`.
152. Ensure every current benchmark result filename contains `0.4-buildfix4`.
153. Remove previous-version release scripts.
154. Remove previous-version buildfix3 docs/audits/tasks/milestone file.
155. Remove transient `__pycache__`.

## Phase K — docs/release
156. Update README.
157. Update Changelog.
158. Update Roadmap.
159. Add architecture document.
160. Add policy-oracle document.
161. Add NumericFast runtime document.
162. Add benchmark contract.
163. Add milestone JSON.
164. Add build audit.
165. Static TOML/JSON/Python/Bash validation.
166. Rust structural/Rustdoc audit.
167. Run cargo check on Rust-enabled host.
168. Run cargo test.
169. Run release build.
170. Run versioned demo.
171. Run full benchmark contract.
172. Package ZIP and SHA-256.
