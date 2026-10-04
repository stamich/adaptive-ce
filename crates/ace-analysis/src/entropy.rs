/// Computes zero-order Shannon entropy in bits per byte from a byte-frequency histogram.
pub fn entropy_h0(frequencies: &[u32; 256], total: usize) -> f32 {
    if total == 0 { return 0.0; }
    frequencies.iter().copied().filter(|&n| n != 0).map(|n| {
        let p = n as f32 / total as f32;
        -p * p.log2()
    }).sum()
}

/// Estimates first-order conditional entropy by sampling adjacent byte transitions.
pub fn sampled_entropy_h1(input: &[u8], stride: usize) -> f32 {
    if input.len() < 2 { return 0.0; }
    let step = stride.max(1);
    let mut transition = vec![0u32; 256 * 256];
    let mut prev = [0u32; 256];
    let mut total = 0usize;
    let mut i = 1usize;
    while i < input.len() {
        let a = input[i - 1] as usize;
        let b = input[i] as usize;
        transition[a * 256 + b] = transition[a * 256 + b].saturating_add(1);
        prev[a] = prev[a].saturating_add(1);
        total += 1;
        i = i.saturating_add(step);
    }
    if total == 0 { return 0.0; }
    let mut h = 0.0f32;
    for a in 0..256 {
        let row_total = prev[a] as usize;
        if row_total == 0 { continue; }
        let p_a = row_total as f32 / total as f32;
        let mut row_h = 0.0f32;
        for b in 0..256 {
            let n = transition[a * 256 + b];
            if n == 0 { continue; }
            let p = n as f32 / row_total as f32;
            row_h -= p * p.log2();
        }
        h += p_a * row_h;
    }
    h
}
