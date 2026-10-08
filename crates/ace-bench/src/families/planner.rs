//! Planner, route, policy-oracle and hot-path families.

use crate::prelude::*;

/// Measures Planner V3.6 quality, work budgets and isolated timing components.
pub(crate) fn planner_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(8);
    let cfg = AceConfig::default();
    let analyzer = DefaultBlockAnalyzer;
    let planner = DefaultCompressionPlanner;
    let blocks = data.chunks(cfg.block_size).collect::<Vec<_>>();

    let mut regret = 0i64;
    let mut global_regret = 0i64;
    let mut global_regret_samples = Vec::<u64>::new();
    let mut generated_recall = 0u64;
    let mut top_k_recall = 0u64;
    let mut top_k_eligible = 0u64;
    let mut sampled_recall = 0u64;
    let mut sampled_eligible = 0u64;
    let mut final_selection_recall = 0u64;
    let mut quality_pool_recall = 0u64;
    let mut quality_pool_eligible = 0u64;
    let mut oracle_top1_after = 0u64;
    let mut oracle_top2_after = 0u64;
    let mut oracle_top3_after = 0u64;
    let mut oracle_rank_analytical_sum = 0u64;
    let mut oracle_rank_stage1_sum = 0u64;
    let mut oracle_rank_after_sum = 0u64;
    let mut oracle_rank_final_sum = 0u64;
    let mut oracle_rank_eligible = 0u64;
    let mut quality_candidates_total = 0u64;
    let mut selected_size_rank_total = 0u64;
    let mut selected_cost_rank_total = 0u64;
    let mut predicted_size_regret_total = 0u64;
    let mut fast_paths = 0u64;
    let mut estimated_total = 0u64;
    let mut sampled_total = 0u64;
    let mut second_stage_total = 0u64;
    let mut full_trial_total = 0u64;
    let mut hybrid_lz_candidates_total = 0u64;
    let mut hybrid_lz_stage1_total = 0u64;
    let mut hybrid_lz_stage2_total = 0u64;
    let mut hybrid_lz_skipped_total = 0u64;
    let mut hybrid_lz_high_confidence_skips_total = 0u64;
    let mut hybrid_lz_sample_bytes_total = 0u64;
    let mut hybrid_lz_max_disagreement_ppm = 0u64;
    let mut regret_samples = Vec::<u64>::new();
    let mut generated_recall_by_class: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    let mut regret_by_class: BTreeMap<&str, (i64, u64)> = BTreeMap::new();
    let mut estimator_by_family: BTreeMap<String, EstimatorErrorStats> = BTreeMap::new();
    let mut estimator_by_class: BTreeMap<String, EstimatorErrorStats> = BTreeMap::new();
    let estimator = DefaultCandidateEstimator;
    let mut details = Vec::new();

    for (idx, block) in blocks.iter().enumerate() {
        let profile = analyzer.analyze(block);
        let candidates = planner.candidates(&profile, &cfg);
        let class = data_class(idx, blocks.len());

        // Diagnostic-only full encodes calibrate the cheap buildfix6 analytical model. They are
        // outside the timed planner hot path and do not count as planner full-trial encodes.
        for candidate in &candidates {
            let estimate = estimator.estimate(candidate, &profile, cfg.profile);
            let actual = encoded_plan_size(block, candidate)? as u64;
            let family = estimator_family_id(candidate);
            estimator_by_family
                .entry(family.clone())
                .or_default()
                .observe(estimate.analytical_size_bytes, actual);
            estimator_by_class
                .entry(format!("{class}:{family}"))
                .or_default()
                .observe(estimate.analytical_size_bytes, actual);
        }

        let planning_context = PlanningContext::classify(block, &cfg);
        let decision = evaluate_candidates_v4_with_route(
            block,
            &profile,
            &candidates,
            &cfg,
            &planning_context.route,
        )?;
        fast_paths += if decision.telemetry.fast_path_hit {
            1
        } else {
            0
        };
        estimated_total += decision.telemetry.estimated_candidates as u64;
        sampled_total += decision.telemetry.sampled_candidates as u64;
        second_stage_total += decision.telemetry.second_stage_candidates as u64;
        full_trial_total += decision.telemetry.full_trial_encodes as u64;
        hybrid_lz_candidates_total += decision.telemetry.hybrid_lz_candidates as u64;
        hybrid_lz_stage1_total += decision.telemetry.hybrid_lz_stage1_candidates as u64;
        hybrid_lz_stage2_total += decision.telemetry.hybrid_lz_stage2_candidates as u64;
        hybrid_lz_skipped_total += decision.telemetry.hybrid_lz_skipped_candidates as u64;
        hybrid_lz_high_confidence_skips_total +=
            decision.telemetry.hybrid_lz_high_confidence_skips as u64;
        hybrid_lz_sample_bytes_total += decision.telemetry.hybrid_lz_sample_bytes as u64;
        hybrid_lz_max_disagreement_ppm =
            hybrid_lz_max_disagreement_ppm.max(decision.telemetry.hybrid_lz_max_disagreement_ppm);
        quality_candidates_total += decision.telemetry.quality_qualified_candidates as u64;
        selected_size_rank_total += decision.telemetry.selected_size_rank as u64;
        selected_cost_rank_total += decision.telemetry.selected_cost_rank as u64;
        predicted_size_regret_total += decision
            .telemetry
            .selected_blended_size_bytes
            .saturating_sub(decision.telemetry.best_blended_size_bytes);

        let route = planning_context.route;
        let global_oracle = oracle_plans()
            .into_iter()
            .map(|plan| {
                let size = encoded_plan_size(block, &plan).unwrap_or(usize::MAX);
                (size, plan)
            })
            .min_by_key(|(size, _)| *size)
            .expect("oracle plans are non-empty");

        let route_oracle = oracle_plans()
            .into_iter()
            .filter(|plan| {
                RoutePolicy::candidate_allowed(route.candidate_route(), plan, &profile, &cfg)
            })
            .map(|plan| {
                let size = encoded_plan_size(block, &plan).unwrap_or(usize::MAX);
                (size, plan)
            })
            .min_by_key(|(size, _)| *size)
            .unwrap_or_else(|| global_oracle.clone());

        let measured_policy_candidates = oracle_plans()
            .into_iter()
            .map(|plan| {
                let size = encoded_plan_size(block, &plan).unwrap_or(usize::MAX) as u64;
                (plan, size)
            })
            .collect::<Vec<_>>();
        let policy_inputs = measured_policy_candidates
            .iter()
            .map(|(plan, size)| PolicyOracleCandidate {
                plan,
                encoded_bytes: *size,
            })
            .collect::<Vec<_>>();
        let policy_decision =
            PolicyOracle::choose(route.candidate_route(), &policy_inputs, &profile, &cfg);
        let policy_oracle = policy_decision
            .map(|decision| {
                (
                    decision.candidate.encoded_bytes as usize,
                    decision.candidate.plan.clone(),
                    decision.preference,
                    decision.reason,
                    decision.accepted_size_loss_bytes,
                )
            })
            .unwrap_or_else(|| {
                (
                    route_oracle.0,
                    route_oracle.1.clone(),
                    ace_planner::CandidatePreference::Neutral,
                    ace_planner::DominanceReason::NoDominance,
                    0,
                )
            });

        let oracle = &(policy_oracle.0, policy_oracle.1.clone());
        let generated_hit = candidates.iter().any(|p| same_plan(p, &oracle.1));
        let top_k_hit = if decision.telemetry.fast_path_hit {
            same_plan(&decision.plan, &oracle.1)
        } else {
            decision.top_k_plans.iter().any(|p| same_plan(p, &oracle.1))
        };
        let sampled_hit = if decision.telemetry.fast_path_hit {
            same_plan(&decision.plan, &oracle.1)
        } else {
            decision
                .final_ranked_plans
                .iter()
                .any(|p| same_plan(p, &oracle.1))
        };
        let final_hit = same_plan(&decision.plan, &oracle.1);
        let quality_pool_hit = if decision.telemetry.fast_path_hit {
            final_hit
        } else {
            decision
                .quality_qualified_plans
                .iter()
                .any(|p| same_plan(p, &oracle.1))
        };
        let oracle_rank_analytical = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .analytical_ranked_plans
                .iter()
                .position(|p| same_plan(p, &oracle.1))
                .map(|i| i + 1)
        };
        let oracle_rank_stage1 = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .stage_one_ranked_plans
                .iter()
                .position(|p| same_plan(p, &oracle.1))
                .map(|i| i + 1)
        };
        let oracle_rank_after = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .final_ranked_plans
                .iter()
                .position(|p| same_plan(p, &oracle.1))
                .map(|i| i + 1)
        };
        let oracle_rank_final = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .quality_qualified_plans
                .iter()
                .position(|p| same_plan(p, &oracle.1))
                .map(|i| i + 1)
        };
        if let (Some(analytical), Some(stage1), Some(after)) = (
            oracle_rank_analytical,
            oracle_rank_stage1,
            oracle_rank_after,
        ) {
            oracle_rank_eligible += 1;
            oracle_rank_analytical_sum += analytical as u64;
            oracle_rank_stage1_sum += stage1 as u64;
            oracle_rank_after_sum += after as u64;
            oracle_rank_final_sum += oracle_rank_final.unwrap_or(0) as u64;
            oracle_top1_after += (after <= 1) as u64;
            oracle_top2_after += (after <= 2) as u64;
            oracle_top3_after += (after <= 3) as u64;
        }

        generated_recall += generated_hit as u64;
        if !decision.telemetry.fast_path_hit {
            top_k_eligible += 1;
            sampled_eligible += 1;
            top_k_recall += top_k_hit as u64;
            sampled_recall += sampled_hit as u64;
        }
        final_selection_recall += final_hit as u64;
        if !decision.telemetry.fast_path_hit {
            quality_pool_eligible += 1;
            quality_pool_recall += quality_pool_hit as u64;
        }

        let selected = decision.plan;
        let selected_size = encoded_plan_size(block, &selected)? as i64;
        let block_regret = selected_size - policy_oracle.0 as i64;
        let block_global_regret = selected_size - global_oracle.0 as i64;
        regret += block_regret;
        global_regret += block_global_regret;
        regret_samples.push(block_regret.max(0) as u64);
        global_regret_samples.push(block_global_regret.max(0) as u64);
        let recall_entry = generated_recall_by_class.entry(class).or_insert((0, 0));
        recall_entry.1 += 1;
        if generated_hit {
            recall_entry.0 += 1;
        }
        let regret_entry = regret_by_class.entry(class).or_insert((0, 0));
        regret_entry.0 += block_regret;
        regret_entry.1 += 1;

        // Build block diagnostics in small compile-time-independent sections. The emitted
        // JSON remains flat and schema-compatible with benchmark contract 1.9.
        let mut identity = JsonObjectBuilder::new();
        identity
            .field("block_id", idx)
            .field("data_class", class)
            .field("candidate_count", candidates.len())
            .field("planner_route", format!("{:?}", route.route))
            .field("generated_oracle", generated_hit)
            .field("top_k_applied", !decision.telemetry.fast_path_hit)
            .field("top_k_oracle", top_k_hit)
            .field("sampled_oracle", sampled_hit);

        let mut ranking = JsonObjectBuilder::new();
        ranking
            .field("oracle_rank_analytical", oracle_rank_analytical)
            .field("oracle_rank_after_stage1", oracle_rank_stage1)
            .field("oracle_rank_after_sampling", oracle_rank_after)
            .field("oracle_rank_final_quality_pool", oracle_rank_final)
            .field("oracle_in_quality_pool", quality_pool_hit)
            .field("selected_is_oracle", final_hit)
            .field("selected_size_rank", decision.telemetry.selected_size_rank)
            .field("selected_cost_rank", decision.telemetry.selected_cost_rank);

        let mut quality = JsonObjectBuilder::new();
        quality
            .field("top_k_count", decision.telemetry.sampled_candidates)
            .field(
                "second_stage_count",
                decision.telemetry.second_stage_candidates,
            )
            .field(
                "quality_qualified_count",
                decision.telemetry.quality_qualified_candidates,
            )
            .field(
                "best_blended_size_bytes",
                decision.telemetry.best_blended_size_bytes,
            )
            .field(
                "quality_limit_bytes",
                decision.telemetry.quality_limit_bytes,
            )
            .field(
                "selected_blended_size_bytes",
                decision.telemetry.selected_blended_size_bytes,
            )
            .field(
                "predicted_size_regret_bytes",
                decision
                    .telemetry
                    .selected_blended_size_bytes
                    .saturating_sub(decision.telemetry.best_blended_size_bytes),
            );

        let mut hybrid = JsonObjectBuilder::new();
        hybrid
            .field(
                "hybrid_lz_candidates",
                decision.telemetry.hybrid_lz_candidates,
            )
            .field(
                "hybrid_lz_stage1_candidates",
                decision.telemetry.hybrid_lz_stage1_candidates,
            )
            .field(
                "hybrid_lz_stage2_candidates",
                decision.telemetry.hybrid_lz_stage2_candidates,
            )
            .field(
                "hybrid_lz_skipped_candidates",
                decision.telemetry.hybrid_lz_skipped_candidates,
            )
            .field(
                "hybrid_lz_high_confidence_skips",
                decision.telemetry.hybrid_lz_high_confidence_skips,
            )
            .field(
                "hybrid_lz_sample_bytes",
                decision.telemetry.hybrid_lz_sample_bytes,
            )
            .field(
                "hybrid_lz_max_disagreement_ppm",
                decision.telemetry.hybrid_lz_max_disagreement_ppm,
            );

        let prediction_error_percent = if selected_size == 0 {
            0.0
        } else {
            (decision.telemetry.selected_blended_size_bytes as f64 - selected_size as f64)
                / selected_size as f64
        };
        let mut outcome = JsonObjectBuilder::new();
        outcome
            .field(
                "selected_prediction_error_bytes",
                decision.telemetry.selected_blended_size_bytes as i64 - selected_size,
            )
            .field(
                "selected_prediction_error_percent",
                prediction_error_percent,
            )
            .field("selected_plan", plan_id(&selected))
            .field("oracle_plan", plan_id(&route_oracle.1))
            .field("route_oracle_plan", plan_id(&route_oracle.1))
            .field("policy_oracle_plan", plan_id(&policy_oracle.1))
            .field("policy_preference", format!("{:?}", policy_oracle.2))
            .field("policy_dominance_reason", format!("{:?}", policy_oracle.3))
            .field("policy_accepted_size_loss_bytes", policy_oracle.4)
            .field("global_oracle_plan", plan_id(&global_oracle.1))
            .field("selected_tier", tier_name(selected.tier))
            .field("selected_bytes", selected_size)
            .field("oracle_bytes", route_oracle.0)
            .field("route_oracle_bytes", route_oracle.0)
            .field("policy_oracle_bytes", policy_oracle.0)
            .field("global_oracle_bytes", global_oracle.0)
            .field("regret_bytes", block_regret)
            .field("route_regret_bytes", selected_size - route_oracle.0 as i64)
            .field("policy_regret_bytes", block_regret)
            .field("global_size_regret_bytes", block_global_regret);

        let mut block_detail = JsonObjectBuilder::new();
        block_detail
            .extend(identity)
            .extend(ranking)
            .extend(quality)
            .extend(hybrid)
            .extend(outcome);
        details.push(block_detail.build());
    }

    let analysis_stats = measure(|| {
        for block in &blocks {
            black_box(analyzer.analyze(block));
        }
        Ok(())
    })?
    .0;
    let fast_analysis_stats = measure(|| {
        for block in &blocks {
            black_box(analyzer.analyze_with_level(block, AnalysisLevel::Fast));
        }
        Ok(())
    })?
    .0;
    let candidate_stats = measure(|| {
        for block in &blocks {
            let p = analyzer.analyze(block);
            black_box(planner.candidates(&p, &cfg));
        }
        Ok(())
    })?
    .0;
    let evaluation_stats = measure(|| {
        for block in &blocks {
            let p = analyzer.analyze(block);
            let c = planner.candidates(&p, &cfg);
            let context = PlanningContext::classify(block, &cfg);
            black_box(evaluate_candidates_v4_with_route(
                block,
                &p,
                &c,
                &cfg,
                &context.route,
            )?);
        }
        Ok(())
    })?
    .0;

    let by_class = generated_recall_by_class
        .into_iter()
        .map(|(k, (hits, total))| {
            (
                k,
                if total == 0 {
                    0.0
                } else {
                    hits as f64 / total as f64
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let regret_classes = regret_by_class
        .into_iter()
        .map(|(k, (bytes, total))| {
            (
                k,
                if total == 0 {
                    0.0
                } else {
                    bytes as f64 / total as f64
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let block_count = blocks.len().max(1) as f64;
    let estimator_calibration = estimator_by_family
        .into_iter()
        .map(|(family, stats)| (family, stats.json()))
        .collect::<BTreeMap<_, _>>();
    let estimator_calibration_by_class = estimator_by_class
        .into_iter()
        .map(|(bucket, stats)| (bucket, stats.json()))
        .collect::<BTreeMap<_, _>>();
    regret_samples.sort_unstable();
    global_regret_samples.sort_unstable();
    let p95_regret_bytes_per_block = if regret_samples.is_empty() {
        0u64
    } else {
        regret_samples[((regret_samples.len() - 1) * 95) / 100]
    };
    let p99_regret_bytes_per_block = if regret_samples.is_empty() {
        0u64
    } else {
        regret_samples[((regret_samples.len() - 1) * 99) / 100]
    };
    let global_p95_regret_bytes_per_block = if global_regret_samples.is_empty() {
        0u64
    } else {
        global_regret_samples[((global_regret_samples.len() - 1) * 95) / 100]
    };
    let global_p99_regret_bytes_per_block = if global_regret_samples.is_empty() {
        0u64
    } else {
        global_regret_samples[((global_regret_samples.len() - 1) * 99) / 100]
    };

    let top_k_rate = if top_k_eligible == 0 {
        1.0
    } else {
        top_k_recall as f64 / top_k_eligible as f64
    };
    let sample_rate = if sampled_eligible == 0 {
        1.0
    } else {
        sampled_recall as f64 / sampled_eligible as f64
    };
    let quality_pool_rate = if quality_pool_eligible == 0 {
        1.0
    } else {
        quality_pool_recall as f64 / quality_pool_eligible as f64
    };
    let rank_denominator = oracle_rank_eligible.max(1) as f64;

    let mut identity = JsonObjectBuilder::new();
    identity
        .field("workload_id", "mixed_8m")
        .field("path", "planner_v4_3_route_aware_oracle")
        .field("blocks", blocks.len());

    let mut quality = JsonObjectBuilder::new();
    quality
        .field("oracle_regret_bytes", regret)
        .field("policy_oracle_regret_bytes", regret)
        .field("policy_regret_bytes_per_block", regret as f64 / block_count)
        .field("route_regret_bytes_per_block", regret as f64 / block_count)
        .field(
            "normalized_regret_bytes_per_block",
            regret as f64 / block_count,
        )
        .field(
            "policy_p95_regret_bytes_per_block",
            p95_regret_bytes_per_block,
        )
        .field(
            "policy_p99_regret_bytes_per_block",
            p99_regret_bytes_per_block,
        )
        .field(
            "route_p95_regret_bytes_per_block",
            p95_regret_bytes_per_block,
        )
        .field(
            "route_p99_regret_bytes_per_block",
            p99_regret_bytes_per_block,
        )
        .field("p95_regret_bytes_per_block", p95_regret_bytes_per_block)
        .field("p99_regret_bytes_per_block", p99_regret_bytes_per_block)
        .field("global_size_regret_bytes", global_regret)
        .field(
            "global_size_regret_bytes_per_block",
            global_regret as f64 / block_count,
        )
        .field(
            "global_p95_regret_bytes_per_block",
            global_p95_regret_bytes_per_block,
        )
        .field(
            "global_p99_regret_bytes_per_block",
            global_p99_regret_bytes_per_block,
        )
        .field("regret_bytes_per_block_by_class", &regret_classes)
        .field("candidate_recall", generated_recall as f64 / block_count)
        .field(
            "candidate_generation_recall",
            generated_recall as f64 / block_count,
        )
        .field(
            "policy_candidate_generation_recall",
            generated_recall as f64 / block_count,
        )
        .field(
            "route_candidate_generation_recall",
            generated_recall as f64 / block_count,
        )
        .field("top_k_recall", top_k_rate)
        .field("policy_top_k_recall", top_k_rate)
        .field("route_top_k_recall", top_k_rate)
        .field("top_k_recall_denominator_blocks", top_k_eligible)
        .field("sample_verifier_recall", sample_rate)
        .field(
            "sample_verifier_recall_denominator_blocks",
            sampled_eligible,
        )
        .field("quality_pool_recall", quality_pool_rate)
        .field(
            "quality_pool_recall_denominator_blocks",
            quality_pool_eligible,
        )
        .field(
            "final_selection_recall",
            final_selection_recall as f64 / block_count,
        )
        .field("candidate_recall_by_class", &by_class);

    let mut ranking = JsonObjectBuilder::new();
    ranking
        .field(
            "oracle_mean_rank_analytical",
            if oracle_rank_eligible == 0 {
                0.0
            } else {
                oracle_rank_analytical_sum as f64 / rank_denominator
            },
        )
        .field(
            "oracle_mean_rank_after_stage1",
            if oracle_rank_eligible == 0 {
                0.0
            } else {
                oracle_rank_stage1_sum as f64 / rank_denominator
            },
        )
        .field(
            "oracle_mean_rank_after_sampling",
            if oracle_rank_eligible == 0 {
                0.0
            } else {
                oracle_rank_after_sum as f64 / rank_denominator
            },
        )
        .field(
            "oracle_mean_rank_final_quality_pool",
            if oracle_rank_eligible == 0 {
                0.0
            } else {
                oracle_rank_final_sum as f64 / rank_denominator
            },
        )
        .field(
            "oracle_top1_rate_after_sampling",
            if oracle_rank_eligible == 0 {
                1.0
            } else {
                oracle_top1_after as f64 / rank_denominator
            },
        )
        .field(
            "oracle_top2_rate_after_sampling",
            if oracle_rank_eligible == 0 {
                1.0
            } else {
                oracle_top2_after as f64 / rank_denominator
            },
        )
        .field(
            "oracle_top3_rate_after_sampling",
            if oracle_rank_eligible == 0 {
                1.0
            } else {
                oracle_top3_after as f64 / rank_denominator
            },
        )
        .field(
            "quality_qualified_candidates_per_block",
            quality_candidates_total as f64 / block_count,
        )
        .field(
            "selected_size_rank_mean",
            selected_size_rank_total as f64 / block_count,
        )
        .field(
            "selected_cost_rank_mean",
            selected_cost_rank_total as f64 / block_count,
        )
        .field(
            "predicted_size_regret_bytes_per_block",
            predicted_size_regret_total as f64 / block_count,
        );

    let mut planner_work = JsonObjectBuilder::new();
    planner_work
        .field("fast_path_rate", fast_paths as f64 / block_count)
        .field(
            "estimated_candidates_per_block",
            estimated_total as f64 / block_count,
        )
        .field(
            "sampled_candidates_per_block",
            sampled_total as f64 / block_count,
        )
        .field(
            "second_stage_candidates_per_block",
            second_stage_total as f64 / block_count,
        )
        .field(
            "full_trial_encodes_per_block",
            full_trial_total as f64 / block_count,
        );

    let mut hybrid = JsonObjectBuilder::new();
    hybrid
        .field(
            "hybrid_lz_candidates_per_block",
            hybrid_lz_candidates_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_stage1_candidates_per_block",
            hybrid_lz_stage1_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_stage2_candidates_per_block",
            hybrid_lz_stage2_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_skipped_candidates_per_block",
            hybrid_lz_skipped_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_high_confidence_skips_per_block",
            hybrid_lz_high_confidence_skips_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_sample_bytes_per_block",
            hybrid_lz_sample_bytes_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_sample_fraction",
            hybrid_lz_sample_bytes_total as f64 / data.len().max(1) as f64,
        )
        .field(
            "hybrid_lz_max_disagreement_ppm",
            hybrid_lz_max_disagreement_ppm,
        );

    let mut calibration = JsonObjectBuilder::new();
    calibration
        .field("estimator_calibration", &estimator_calibration)
        .field(
            "estimator_calibration_by_data_class",
            &estimator_calibration_by_class,
        );

    let mut timing = JsonObjectBuilder::new();
    timing
        .value(
            "analysis_per_file",
            timing_json(&analysis_stats, data.len()),
        )
        .value(
            "fast_analysis_per_file",
            timing_json(&fast_analysis_stats, data.len()),
        )
        .value(
            "candidate_generation_per_file",
            timing_json(&candidate_stats, data.len()),
        )
        .value(
            "evaluation_per_file",
            timing_json(&evaluation_stats, data.len()),
        );

    let mut row = JsonObjectBuilder::new();
    row.extend(identity)
        .extend(quality)
        .extend(ranking)
        .extend(planner_work)
        .extend(hybrid)
        .extend(calibration)
        .value("timing", timing.build())
        .field("block_details", details);

    Ok(vec![row.build()])
}

/// Benchmarks the low-cost Planner V4.3 route classifier independently of generic analysis.
pub(crate) fn planner_route_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let workloads = [
        (
            "u32-counter",
            workload_bytes("u32-counter", 16 * 1024 * 1024),
        ),
        (
            "u64-timestamps",
            workload_bytes("u64-timestamps", 16 * 1024 * 1024),
        ),
        (
            "delta-variable",
            workload_bytes("delta-variable", 16 * 1024 * 1024),
        ),
        ("mixed", mixed_data(16)),
        ("random", workload_bytes("random", 16 * 1024 * 1024)),
    ];
    let mut rows = Vec::new();
    for (kind, data) in workloads {
        let cfg = AceConfig {
            profile: CompressionProfile::Balanced,
            threads: 1,
            ..AceConfig::default()
        };
        let prefilter = numeric_prefilter(&data);
        let (stats, route) = measure(|| Ok(classify_planner_route(black_box(&data), &cfg)))?;
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", kind)
            .field("path", "planner-v4.3-route")
            .field("route", route.route.label())
            .field("reason", format!("{:?}", route.reason))
            .field("prefilter_confidence", prefilter.confidence)
            .field("prefilter_likely_numeric", prefilter.likely_numeric)
            .field("prefilter_strong_numeric", prefilter.strong_numeric)
            .field("prefilter_width", format!("{:?}", prefilter.width_hint))
            .value("route_timing", timing_json(&stats, data.len()));
        rows.push(row.build());
    }
    Ok(rows)
}

/// Compares the diagnostic global size oracle with the production route-aware oracle.
///
/// The benchmark deliberately full-encodes oracle candidates outside the planner hot path. It is
/// therefore a measurement/validation tool, not a production planning algorithm.
pub(crate) fn policy_oracle_v2_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let workloads = [
        ("zeros", workload_bytes("zeros", 4 * 1024 * 1024)),
        ("runs", workload_bytes("runs", 4 * 1024 * 1024)),
        (
            "u32-counter",
            workload_bytes("u32-counter", 4 * 1024 * 1024),
        ),
        (
            "u64-timestamps",
            workload_bytes("u64-timestamps", 4 * 1024 * 1024),
        ),
        (
            "delta-variable",
            workload_bytes("delta-variable", 4 * 1024 * 1024),
        ),
        (
            "structured",
            workload_bytes("structured-json", 4 * 1024 * 1024),
        ),
        ("random", workload_bytes("random", 4 * 1024 * 1024)),
        ("mixed", mixed_data(4)),
    ];
    let analyzer = DefaultBlockAnalyzer;
    let planner = DefaultCompressionPlanner;
    let cfg = AceConfig::default();
    let mut rows = Vec::new();

    for (workload_id, data) in workloads {
        let block = &data[..data.len().min(cfg.block_size)];
        let profile = analyzer.analyze(block);
        let planning_context = PlanningContext::classify(block, &cfg);
        let route = planning_context.route;
        let candidates = planner.candidates(&profile, &cfg);
        let decision =
            evaluate_candidates_v4_with_route(block, &profile, &candidates, &cfg, &route)?;

        let global = oracle_plans()
            .into_iter()
            .map(|plan| (encoded_plan_size(block, &plan).unwrap_or(usize::MAX), plan))
            .min_by_key(|(size, _)| *size)
            .expect("global oracle");
        let route_oracle = oracle_plans()
            .into_iter()
            .filter(|plan| {
                RoutePolicy::candidate_allowed(route.candidate_route(), plan, &profile, &cfg)
            })
            .map(|plan| (encoded_plan_size(block, &plan).unwrap_or(usize::MAX), plan))
            .min_by_key(|(size, _)| *size)
            .expect("route oracle");
        let measured_policy_candidates = oracle_plans()
            .into_iter()
            .map(|plan| {
                let size = encoded_plan_size(block, &plan).unwrap_or(usize::MAX) as u64;
                (plan, size)
            })
            .collect::<Vec<_>>();
        let policy_inputs = measured_policy_candidates
            .iter()
            .map(|(plan, size)| PolicyOracleCandidate {
                plan,
                encoded_bytes: *size,
            })
            .collect::<Vec<_>>();
        let policy = PolicyOracle::choose(route.candidate_route(), &policy_inputs, &profile, &cfg)
            .expect("policy oracle");
        let selected_size = encoded_plan_size(block, &decision.plan)?;

        let allowed = candidates
            .iter()
            .filter(|plan| {
                matches!(
                    RoutePolicy::candidate_eligibility(
                        route.candidate_route(),
                        plan,
                        &profile,
                        &cfg
                    ),
                    CandidateEligibility::Allowed
                )
            })
            .count();
        let diagnostic = candidates.len().saturating_sub(allowed);

        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", workload_id)
            .field("path", "policy-oracle-v2")
            .field("planner_route", format!("{:?}", route.route))
            .field("selected_plan", plan_id(&decision.plan))
            .field("global_oracle_plan", plan_id(&global.1))
            .field("route_oracle_plan", plan_id(&route_oracle.1))
            .field("policy_oracle_plan", plan_id(policy.candidate.plan))
            .field("policy_preference", format!("{:?}", policy.preference))
            .field("policy_dominance_reason", format!("{:?}", policy.reason))
            .field(
                "policy_accepted_size_loss_bytes",
                policy.accepted_size_loss_bytes,
            )
            .field("selected_bytes", selected_size)
            .field("global_oracle_bytes", global.0)
            .field("route_oracle_bytes", route_oracle.0)
            .field("policy_oracle_bytes", policy.candidate.encoded_bytes)
            .field(
                "global_size_regret_bytes",
                selected_size as i64 - global.0 as i64,
            )
            .field(
                "route_regret_bytes",
                selected_size as i64 - route_oracle.0 as i64,
            )
            .field(
                "policy_regret_bytes",
                selected_size as i64 - policy.candidate.encoded_bytes as i64,
            )
            .field("allowed_candidates", allowed)
            .field("diagnostic_only_candidates", diagnostic);
        rows.push(row.build());
    }
    Ok(rows)
}

/// Breaks end-to-end compression time into route, generic-analysis, planning and encode stages.
///
/// The benchmark is intentionally end-to-end so stage sums can be compared with observed wall
/// clock throughput while still revealing accidental duplicate classification/analysis work.
pub(crate) fn planner_hotpath_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let workloads = [
        ("mixed-fast", CompressionProfile::Fast, mixed_data(16)),
        (
            "mixed-balanced",
            CompressionProfile::Balanced,
            mixed_data(16),
        ),
        (
            "random",
            CompressionProfile::Balanced,
            workload_bytes("random", 16 * 1024 * 1024),
        ),
        (
            "structured",
            CompressionProfile::Balanced,
            workload_bytes("structured-json", 16 * 1024 * 1024),
        ),
        (
            "zeros",
            CompressionProfile::Balanced,
            workload_bytes("zeros", 16 * 1024 * 1024),
        ),
        (
            "u32-counter",
            CompressionProfile::Balanced,
            workload_bytes("u32-counter", 16 * 1024 * 1024),
        ),
        (
            "u64-timestamps",
            CompressionProfile::Balanced,
            workload_bytes("u64-timestamps", 16 * 1024 * 1024),
        ),
    ];
    let mut rows = Vec::new();

    for (workload_id, profile, data) in workloads {
        let cfg = AceConfig {
            profile,
            threads: 1,
            ..AceConfig::default()
        };
        let engine = AceEngine::new(cfg.clone())?;
        let route = RoutePolicy::classify(&data[..data.len().min(cfg.block_size)], &cfg);
        let (wall, encoded) = measure(|| Ok(engine.compress(&data)?))?;
        let (_, stats) = engine.compress_with_stats(&data)?;
        assert_eq!(engine.decompress(&encoded)?, data);

        let total_stage_ns = stats
            .route_classify_time
            .saturating_add(stats.generic_analysis_time)
            .saturating_add(stats.planning_time)
            .saturating_add(stats.encoding_time)
            .as_nanos()
            .min(u64::MAX as u128) as u64;

        let mut stage = JsonObjectBuilder::new();
        stage
            .field(
                "route_classify_ns",
                stats.route_classify_time.as_nanos().min(u64::MAX as u128) as u64,
            )
            .field(
                "generic_analysis_ns",
                stats.generic_analysis_time.as_nanos().min(u64::MAX as u128) as u64,
            )
            .field(
                "planning_ns",
                stats.planning_time.as_nanos().min(u64::MAX as u128) as u64,
            )
            .field(
                "encoding_ns",
                stats.encoding_time.as_nanos().min(u64::MAX as u128) as u64,
            )
            .field("total_stage_ns", total_stage_ns)
            .field("fast_path_blocks", stats.planner_fast_path_blocks)
            .field("estimated_candidates", stats.planner_estimated_candidates)
            .field("sampled_candidates", stats.planner_sampled_candidates);

        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", workload_id)
            .field("path", "planner-v4.3-hotpath")
            .field("route_first_block", format!("{:?}", route.route))
            .field("input_bytes", data.len())
            .field("compressed_bytes", encoded.len())
            .field(
                "compression_ratio",
                data.len() as f64 / encoded.len().max(1) as f64,
            )
            .value("stage_timing", stage.build())
            .value("wall_clock", timing_json(&wall, data.len()));
        rows.push(row.build());
    }
    Ok(rows)
}
