//! Scoring primitives for attacks and anonymity sets.
//!
//! Two families live here. `roc_auc` scores an *attack*: how well a pairwise
//! linkage score separates same-operator pairs from different-operator pairs.
//! The entropy functions score a *defence*: how much genuine uncertainty a
//! payout's provenance actually carries, which is usually well below the
//! nominal `k` a protocol advertises.

/// Area under the ROC curve for a set of scored, labelled samples.
///
/// Computed by the rank-sum (Mann-Whitney U) identity rather than by
/// integrating a sampled curve, so the result is exact and ties are handled by
/// averaging ranks instead of being broken arbitrarily. Tie handling is not a
/// detail here: a defence that makes every pair score identically should
/// measure 0.5, and a curve-integrating implementation can report 1.0 for that
/// same input depending on sort order.
///
/// Returns `None` when either class is absent, since AUC is undefined then.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn roc_auc(samples: &[(f64, bool)]) -> Option<f64> {
    let positives = samples.iter().filter(|(_, label)| *label).count();
    let negatives = samples.len() - positives;
    if positives == 0 || negatives == 0 {
        return None;
    }

    let mut ordered: Vec<(f64, bool)> = samples.to_vec();
    ordered.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Average ranks within each run of equal scores.
    let mut positive_rank_sum = 0.0_f64;
    let mut index = 0_usize;
    while index < ordered.len() {
        let mut end = index + 1;
        while end < ordered.len() && (ordered[end].0 - ordered[index].0).abs() < f64::EPSILON {
            end += 1;
        }
        // Ranks are 1-based; the average of the ranks index+1..=end.
        let average_rank = ((index + 1 + end) as f64) / 2.0;
        for sample in &ordered[index..end] {
            if sample.1 {
                positive_rank_sum += average_rank;
            }
        }
        index = end;
    }

    let positives = positives as f64;
    let negatives = negatives as f64;
    Some((positive_rank_sum - positives * (positives + 1.0) / 2.0) / (positives * negatives))
}

/// Shannon entropy, in bits, of a discrete distribution.
///
/// Input weights need not be normalised; zero and negative weights are ignored.
#[must_use]
pub fn shannon_entropy(weights: &[f64]) -> f64 {
    let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
    if total <= 0.0 {
        return 0.0;
    }
    -weights
        .iter()
        .filter(|w| **w > 0.0)
        .map(|weight| {
            let probability = weight / total;
            probability * probability.log2()
        })
        .sum::<f64>()
}

/// Min-entropy, in bits: the guessing advantage of an adversary who always
/// picks the single most likely candidate.
///
/// Reported alongside Shannon entropy because the two diverge exactly when a
/// distribution is skewed — which is the case that matters. A pool of 64 with
/// one dominant funder has a comfortable Shannon entropy and almost no
/// min-entropy, and it is min-entropy that bounds the real attack.
#[must_use]
pub fn min_entropy(weights: &[f64]) -> f64 {
    let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
    if total <= 0.0 {
        return 0.0;
    }
    let peak = weights
        .iter()
        .copied()
        .filter(|w| *w > 0.0)
        .fold(0.0_f64, f64::max);
    -(peak / total).log2()
}

/// Effective anonymity set size implied by an entropy in bits.
///
/// `2^H`, following Serjantov-Danezis: the size of the uniform set that would
/// leave an adversary equally uncertain. Comparable to the nominal `k` a
/// protocol advertises, and usually smaller.
#[must_use]
pub fn effective_set_size(entropy_bits: f64) -> f64 {
    entropy_bits.exp2()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: f64 = 1e-9;

    #[test]
    fn perfect_separation_scores_one() {
        let samples = [(0.9, true), (0.8, true), (0.2, false), (0.1, false)];
        let auc = roc_auc(&samples).expect("both classes present");
        assert!((auc - 1.0).abs() < TOLERANCE, "got {auc}");
    }

    #[test]
    fn perfectly_inverted_separation_scores_zero() {
        let samples = [(0.1, true), (0.2, true), (0.8, false), (0.9, false)];
        let auc = roc_auc(&samples).expect("both classes present");
        assert!(auc.abs() < TOLERANCE, "got {auc}");
    }

    #[test]
    fn total_ties_score_one_half() {
        // The case that separates a correct implementation from a naive one: a
        // defence that collapses every score to the same value is exactly no
        // better and no worse than guessing.
        let samples = [(0.5, true), (0.5, false), (0.5, true), (0.5, false)];
        let auc = roc_auc(&samples).expect("both classes present");
        assert!((auc - 0.5).abs() < TOLERANCE, "got {auc}");
    }

    #[test]
    fn partial_ties_are_rank_averaged() {
        // AUC is the probability a random positive outranks a random negative,
        // with ties counting a half. Enumerating the four (positive, negative)
        // pairs: 0.9>0.5, 0.9>0.1, 0.5==0.5 (half), 0.5>0.1 => 3.5/4.
        let samples = [(0.9, true), (0.5, true), (0.5, false), (0.1, false)];
        let auc = roc_auc(&samples).expect("both classes present");
        assert!((auc - 0.875).abs() < TOLERANCE, "got {auc}");
    }

    #[test]
    fn single_class_input_is_undefined() {
        assert!(roc_auc(&[(0.5, true), (0.9, true)]).is_none());
        assert!(roc_auc(&[]).is_none());
    }

    #[test]
    fn uniform_distribution_entropies_agree() {
        let uniform = [1.0_f64; 8];
        assert!((shannon_entropy(&uniform) - 3.0).abs() < TOLERANCE);
        assert!((min_entropy(&uniform) - 3.0).abs() < TOLERANCE);
        assert!((effective_set_size(shannon_entropy(&uniform)) - 8.0).abs() < TOLERANCE);
    }

    #[test]
    fn skew_collapses_min_entropy_faster_than_shannon() {
        // 64 nominal members, one holding 90% of the mass.
        let mut weights = vec![0.1 / 63.0; 63];
        weights.push(0.9);
        let shannon = shannon_entropy(&weights);
        let min = min_entropy(&weights);
        assert!(min < shannon, "min {min} should trail shannon {shannon}");
        // Nominal k = 64, but the adversary guesses right ~90% of the time.
        assert!(
            effective_set_size(min) < 1.2,
            "effective set {} should be near 1",
            effective_set_size(min)
        );
    }

    #[test]
    fn degenerate_distributions_do_not_panic() {
        assert!((shannon_entropy(&[]) - 0.0).abs() < TOLERANCE);
        assert!((min_entropy(&[]) - 0.0).abs() < TOLERANCE);
        assert!((shannon_entropy(&[0.0, -1.0]) - 0.0).abs() < TOLERANCE);
        assert!((min_entropy(&[0.0, -1.0]) - 0.0).abs() < TOLERANCE);
    }
}
