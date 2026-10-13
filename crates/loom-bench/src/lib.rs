//! Scoring and statistics for the ADR-140 P1 pilot.
//!
//! The pilot compares two serving regimes of one corpus through one door:
//! arm 3 injects the scaffold, arm 4 lets the model drive `/mcp`. Everything
//! here is deterministic and offline so that a preregistered scoring rule can
//! be re-run on stored run records.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use serde::{Deserialize, Serialize};

/// One preregistered question.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Question {
    /// Stable id, `q001`...
    pub id: String,
    /// `lookup`, `cross` or `path`.
    pub stratum: String,
    /// Domain of the first class, the clustering unit for the CI.
    pub domain: String,
    /// The user prompt, sent verbatim to both arms.
    pub prompt: String,
    /// Gold concept titles; every one is expected in the answer.
    pub gold: Vec<String>,
}

/// One arm's run on one question.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunRecord {
    /// Question id.
    pub id: String,
    /// Stratum copied from the question.
    pub stratum: String,
    /// `arm3` or `arm4`.
    pub arm: String,
    /// Position of this arm within the question (0 or 1), after randomisation.
    pub order: u8,
    /// `/loom/generation` id observed before the call.
    pub generation: String,
    /// Final answer text.
    pub answer: String,
    /// Tool calls executed (arm 4).
    pub tool_calls: u32,
    /// Model round trips.
    pub turns: u32,
    /// Summed `usage.prompt_tokens`.
    pub prompt_tokens: u64,
    /// Summed `usage.completion_tokens`.
    pub completion_tokens: u64,
    /// Tokens the façade reports injecting (arm 3): an estimate, already inside `prompt_tokens`.
    pub injected_tokens: u64,
    /// Wall-clock milliseconds for the whole arm.
    pub wall_ms: u64,
    /// The tool-call cap stopped the run before a final answer.
    pub capped: bool,
    /// `loom.served_mode` for arm 3 (`verbatim` means the model never ran).
    pub served_mode: String,
    /// Transport or protocol error, if any; the run still scores (as a miss).
    pub error: Option<String>,
}

/// Lower-case, alphanumerics only, single-spaced, padded so whole words match.
#[must_use]
pub fn normalise(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(' ');
    let mut gap = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            gap = false;
        } else if !gap {
            out.push(' ');
            gap = true;
        }
    }
    if !gap {
        out.push(' ');
    }
    out
}

/// Fraction of gold titles that occur, as whole words, in the answer.
#[must_use]
pub fn recall(answer: &str, gold: &[String]) -> f64 {
    if gold.is_empty() {
        return 0.0;
    }
    let hay = normalise(answer);
    let hits = gold
        .iter()
        .filter(|g| {
            let n = normalise(g);
            !n.trim().is_empty() && hay.contains(&n)
        })
        .count();
    #[allow(clippy::cast_precision_loss)]
    {
        hits as f64 / gold.len() as f64
    }
}

/// splitmix64: a tiny seeded generator, enough for shuffles and bootstraps.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Seeded generator.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `0..n` (`n > 0`).
    #[allow(clippy::cast_possible_truncation)]
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            v.swap(i, self.below(i + 1));
        }
    }
}

/// A paired observation: the arm 4 minus arm 3 difference for one question.
#[derive(Clone, Debug)]
pub struct Paired {
    /// Clustering unit (domain).
    pub cluster: String,
    /// `arm4 - arm3` on the metric.
    pub delta: f64,
}

/// Mean of the deltas.
#[must_use]
pub fn mean_delta(p: &[Paired]) -> f64 {
    if p.is_empty() {
        return f64::NAN;
    }
    #[allow(clippy::cast_precision_loss)]
    {
        p.iter().map(|x| x.delta).sum::<f64>() / p.len() as f64
    }
}

/// 95% percentile bootstrap CI of the mean delta.
///
/// With `clustered`, whole domains are resampled and then questions within
/// each drawn domain, which is the protocol's primary interval because
/// questions cluster by domain. Otherwise questions are resampled singly.
#[must_use]
pub fn bootstrap_ci(p: &[Paired], clustered: bool, resamples: usize, seed: u64) -> (f64, f64) {
    if p.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let mut rng = Rng::new(seed);
    let mut groups: Vec<Vec<f64>> = Vec::new();
    {
        let mut names: Vec<&str> = Vec::new();
        for x in p {
            let i = names
                .iter()
                .position(|n| *n == x.cluster)
                .unwrap_or_else(|| {
                    names.push(&x.cluster);
                    groups.push(Vec::new());
                    names.len() - 1
                });
            groups[i].push(x.delta);
        }
    }
    let mut means = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let (mut sum, mut n) = (0.0_f64, 0_usize);
        if clustered {
            for _ in 0..groups.len() {
                let g = &groups[rng.below(groups.len())];
                for _ in 0..g.len() {
                    sum += g[rng.below(g.len())];
                    n += 1;
                }
            }
        } else {
            for _ in 0..p.len() {
                sum += p[rng.below(p.len())].delta;
                n += 1;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        means.push(sum / n as f64);
    }
    means.sort_by(f64::total_cmp);
    let lo = means[(resamples as f64 * 0.025) as usize];
    let hi = means[((resamples as f64 * 0.975) as usize).min(resamples - 1)];
    (lo, hi)
}

/// Median of a slice (`NaN` when empty).
#[must_use]
pub fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let m = s.len() / 2;
    if s.len() % 2 == 1 {
        s[m]
    } else {
        f64::midpoint(s[m - 1], s[m])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_matches_whole_words_case_insensitively() {
        let gold = vec!["AI Application".to_owned(), "Healthcare".to_owned()];
        assert!((recall("The parent is ai application.", &gold) - 0.5).abs() < 1e-9);
        assert!((recall("healthcare; AI-Application", &gold) - 1.0).abs() < 1e-9);
        assert!(
            recall("unhealthcare", &gold) < 1e-9,
            "substring must not match"
        );
    }

    #[test]
    fn recall_of_nothing_is_zero() {
        assert!(recall("anything", &[]) < 1e-9);
    }

    #[test]
    fn bootstrap_is_deterministic_and_brackets_the_mean() {
        let p: Vec<Paired> = (0..30)
            .map(|i| Paired {
                cluster: format!("d{}", i % 6),
                delta: f64::from(i % 5) * 0.1,
            })
            .collect();
        let a = bootstrap_ci(&p, true, 2000, 7);
        assert_eq!(a, bootstrap_ci(&p, true, 2000, 7));
        let m = mean_delta(&p);
        assert!(a.0 <= m && m <= a.1, "{a:?} vs {m}");
        let naive = bootstrap_ci(&p, false, 2000, 7);
        assert!(a.1 - a.0 >= (naive.1 - naive.0) * 0.5);
    }

    #[test]
    fn shuffle_is_a_seeded_permutation() {
        let mut a: Vec<u32> = (0..20).collect();
        let mut b = a.clone();
        Rng::new(3).shuffle(&mut a);
        Rng::new(3).shuffle(&mut b);
        assert_eq!(a, b);
        a.sort_unstable();
        assert_eq!(a, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn median_handles_even_and_odd() {
        assert!((median(&[3.0, 1.0, 2.0]) - 2.0).abs() < 1e-9);
        assert!((median(&[1.0, 2.0, 3.0, 4.0]) - 2.5).abs() < 1e-9);
    }
}
