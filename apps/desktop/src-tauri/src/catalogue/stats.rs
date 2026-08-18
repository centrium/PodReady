use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Continuous baseline metric containing robust descriptive statistics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContinuousBaselineMetric {
    pub id: String,
    pub label: String,
    pub unit: String,
    pub sample_count: usize,
    pub median: f64,
    pub q1: f64,
    pub q3: f64,
    pub min: f64,
    pub max: f64,
}

/// Distribution item for categorical baseline metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CategoricalDistributionItem {
    pub value: String,
    pub count: usize,
    pub proportion: f64,
}

/// Categorical baseline metric containing dominant modal value and distribution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CategoricalBaselineMetric {
    pub id: String,
    pub label: String,
    pub sample_count: usize,
    pub dominant_value: String,
    pub dominant_count: usize,
    pub dominant_proportion: f64,
    pub distribution: Vec<CategoricalDistributionItem>,
}

/// Computes percentile using Hyndman-Fan Method 7 (linear interpolation).
///
/// Given sorted array X of length N:
/// For p in [0.0, 1.0]:
/// Rank h = (N - 1) * p
/// Let k = floor(h), fraction f = h - k.
/// Result = X[k] + f * (X[k + 1] - X[k])
///
/// Behavior:
/// - N = 0: None
/// - N = 1: X[0]
/// - Median (p = 0.5):
///   - Odd N: exact middle element
///   - Even N: mean of two middle elements
/// - Q1 (p = 0.25) & Q3 (p = 0.75): smooth linear interpolation
pub fn calculate_percentile_r7(sorted_values: &[f64], p: f64) -> Option<f64> {
    let n = sorted_values.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(sorted_values[0]);
    }
    let p_clamped = p.clamp(0.0, 1.0);
    let h = (n - 1) as f64 * p_clamped;
    let k = h.floor() as usize;
    let f = h - (k as f64);

    if k >= n - 1 {
        Some(sorted_values[n - 1])
    } else {
        Some(sorted_values[k] + f * (sorted_values[k + 1] - sorted_values[k]))
    }
}

/// Calculates robust continuous statistics for a slice of f64 values.
pub fn calculate_continuous_metric(
    values: &[f64],
    id: &str,
    label: &str,
    unit: &str,
) -> Option<ContinuousBaselineMetric> {
    if values.is_empty() {
        return None;
    }

    let mut sorted = values.to_vec();
    // Sort handling NaNs safely
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let median = calculate_percentile_r7(&sorted, 0.5)?;
    let q1 = calculate_percentile_r7(&sorted, 0.25)?;
    let q3 = calculate_percentile_r7(&sorted, 0.75)?;

    Some(ContinuousBaselineMetric {
        id: id.to_string(),
        label: label.to_string(),
        unit: unit.to_string(),
        sample_count: sorted.len(),
        median,
        q1,
        q3,
        min,
        max,
    })
}

/// Calculates categorical baseline statistics (mode, dominant proportion, and sorted distribution).
pub fn calculate_categorical_metric<T: Clone + Eq + std::hash::Hash + std::fmt::Display>(
    items: &[T],
    id: &str,
    label: &str,
) -> Option<CategoricalBaselineMetric> {
    if items.is_empty() {
        return None;
    }

    let total = items.len();
    let mut counts: HashMap<String, usize> = HashMap::new();

    for item in items {
        let key = item.to_string();
        *counts.entry(key).or_insert(0) += 1;
    }

    // Sort distribution by count DESC, then alphabetically by value ASC for determinism
    let mut distribution: Vec<CategoricalDistributionItem> = counts
        .into_iter()
        .map(|(value, count)| CategoricalDistributionItem {
            proportion: (count as f64) / (total as f64),
            value,
            count,
        })
        .collect();

    distribution.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.value.cmp(&b.value))
    });

    let dominant = &distribution[0];

    Some(CategoricalBaselineMetric {
        id: id.to_string(),
        label: label.to_string(),
        sample_count: total,
        dominant_value: dominant.value.clone(),
        dominant_count: dominant.count,
        dominant_proportion: dominant.proportion,
        distribution,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile_r7_single_value() {
        let values = vec![42.0];
        assert_eq!(calculate_percentile_r7(&values, 0.0), Some(42.0));
        assert_eq!(calculate_percentile_r7(&values, 0.5), Some(42.0));
        assert_eq!(calculate_percentile_r7(&values, 1.0), Some(42.0));
    }

    #[test]
    fn test_percentile_r7_odd_count() {
        // Sorted: [10.0, 20.0, 30.0, 40.0, 50.0] (N=5)
        // h for p=0.5: (5-1)*0.5 = 2.0 -> index 2 -> 30.0
        // h for p=0.25: (5-1)*0.25 = 1.0 -> index 1 -> 20.0
        // h for p=0.75: (5-1)*0.75 = 3.0 -> index 3 -> 40.0
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        assert_eq!(calculate_percentile_r7(&values, 0.5), Some(30.0));
        assert_eq!(calculate_percentile_r7(&values, 0.25), Some(20.0));
        assert_eq!(calculate_percentile_r7(&values, 0.75), Some(40.0));
    }

    #[test]
    fn test_percentile_r7_even_count() {
        // Sorted: [10.0, 20.0, 30.0, 40.0] (N=4)
        // h for p=0.5: (4-1)*0.5 = 1.5 -> X[1] + 0.5*(X[2]-X[1]) = 20.0 + 0.5*(10.0) = 25.0
        // h for p=0.25: (4-1)*0.25 = 0.75 -> X[0] + 0.75*(X[1]-X[0]) = 10.0 + 0.75*10.0 = 17.5
        // h for p=0.75: (4-1)*0.75 = 2.25 -> X[2] + 0.25*(X[3]-X[2]) = 30.0 + 0.25*10.0 = 32.5
        let values = vec![10.0, 20.0, 30.0, 40.0];
        assert_eq!(calculate_percentile_r7(&values, 0.5), Some(25.0));
        assert_eq!(calculate_percentile_r7(&values, 0.25), Some(17.5));
        assert_eq!(calculate_percentile_r7(&values, 0.75), Some(32.5));
    }

    #[test]
    fn test_negative_lufs_quartiles() {
        // Sorted: [-18.0, -16.5, -16.0, -15.5, -14.0] (N=5)
        let values = vec![-18.0, -16.5, -16.0, -15.5, -14.0];
        let metric = calculate_continuous_metric(&values, "loudness", "Loudness", "LUFS").unwrap();
        assert_eq!(metric.median, -16.0);
        assert_eq!(metric.q1, -16.5);
        assert_eq!(metric.q3, -15.5);
        assert_eq!(metric.min, -18.0);
        assert_eq!(metric.max, -14.0);
        assert_eq!(metric.sample_count, 5);
    }

    #[test]
    fn test_categorical_dominant_mode() {
        let formats = vec!["MP3", "MP3", "MP3", "WAV", "M4A"];
        let metric = calculate_categorical_metric(&formats, "format", "Format").unwrap();
        assert_eq!(metric.dominant_value, "MP3");
        assert_eq!(metric.dominant_count, 3);
        assert_eq!(metric.dominant_proportion, 0.6);
        assert_eq!(metric.sample_count, 5);
        assert_eq!(metric.distribution.len(), 3);
        assert_eq!(metric.distribution[0].value, "MP3");
        assert_eq!(metric.distribution[0].count, 3);
    }
}
