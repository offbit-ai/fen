//! Baseline statistics and trend analysis types for statistical anomaly detection.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type-safe baseline ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BaselineId(pub Uuid);

impl BaselineId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for BaselineId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BaselineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type-safe anomaly record ID (for persisted anomalies)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnomalyId(pub Uuid);

impl AnomalyId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for AnomalyId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AnomalyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Statistical baseline for a vendor/metric combination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineStats {
    /// Number of samples in the baseline
    pub count: u64,
    /// Arithmetic mean
    pub mean: f64,
    /// Standard deviation
    pub stddev: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// 25th percentile
    pub p25: f64,
    /// 50th percentile (median)
    pub p50: f64,
    /// 75th percentile
    pub p75: f64,
    /// 90th percentile
    pub p90: f64,
    /// 95th percentile
    pub p95: f64,
    /// 99th percentile
    pub p99: f64,
}

impl BaselineStats {
    /// Create empty baseline stats
    pub fn empty() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            stddev: 0.0,
            min: 0.0,
            max: 0.0,
            p25: 0.0,
            p50: 0.0,
            p75: 0.0,
            p90: 0.0,
            p95: 0.0,
            p99: 0.0,
        }
    }

    /// Compute baseline stats from a slice of values
    pub fn from_values(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self::empty();
        }

        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;

        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let stddev = variance.sqrt();

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min = sorted.first().copied().unwrap_or(0.0);
        let max = sorted.last().copied().unwrap_or(0.0);

        Self {
            count: values.len() as u64,
            mean,
            stddev,
            min,
            max,
            p25: percentile_at(&sorted, 25.0),
            p50: percentile_at(&sorted, 50.0),
            p75: percentile_at(&sorted, 75.0),
            p90: percentile_at(&sorted, 90.0),
            p95: percentile_at(&sorted, 95.0),
            p99: percentile_at(&sorted, 99.0),
        }
    }

    /// Compute z-score for a given value
    pub fn z_score(&self, value: f64) -> f64 {
        if self.stddev == 0.0 {
            return 0.0;
        }
        (value - self.mean) / self.stddev
    }

    /// Compute approximate percentile for a value using linear interpolation
    pub fn percentile_of(&self, value: f64) -> f64 {
        let percentile_points = [
            (0.0, self.min),
            (25.0, self.p25),
            (50.0, self.p50),
            (75.0, self.p75),
            (90.0, self.p90),
            (95.0, self.p95),
            (99.0, self.p99),
            (100.0, self.max),
        ];

        for window in percentile_points.windows(2) {
            let (p1, v1) = window[0];
            let (p2, v2) = window[1];
            if value <= v2 {
                if (v2 - v1).abs() < f64::EPSILON {
                    return p1;
                }
                return p1 + (p2 - p1) * (value - v1) / (v2 - v1);
            }
        }
        100.0
    }
}

/// Compute percentile at a given percentage from sorted values
fn percentile_at(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Trend indicator for time-series analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendIndicator {
    /// Values are increasing over time
    Increasing,
    /// Values are decreasing over time
    Decreasing,
    /// Values are stable (low slope)
    Stable,
    /// Values are volatile (high variance)
    Volatile,
}

impl TrendIndicator {
    /// Detect trend from recent values (oldest first)
    pub fn from_values(values: &[f64]) -> Self {
        if values.len() < 3 {
            return TrendIndicator::Stable;
        }

        let n = values.len() as f64;
        let x_mean = (n - 1.0) / 2.0;
        let y_mean: f64 = values.iter().sum::<f64>() / n;

        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }

        if denominator == 0.0 {
            return TrendIndicator::Stable;
        }

        let slope = numerator / denominator;
        let normalized_slope = slope / y_mean.abs().max(1.0);

        // Check volatility (coefficient of variation)
        let variance: f64 = values.iter().map(|&v| (v - y_mean).powi(2)).sum::<f64>() / n;
        let cv = variance.sqrt() / y_mean.abs().max(1.0);

        if cv > 0.5 {
            TrendIndicator::Volatile
        } else if normalized_slope > 0.1 {
            TrendIndicator::Increasing
        } else if normalized_slope < -0.1 {
            TrendIndicator::Decreasing
        } else {
            TrendIndicator::Stable
        }
    }
}

impl std::fmt::Display for TrendIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrendIndicator::Increasing => write!(f, "Increasing"),
            TrendIndicator::Decreasing => write!(f, "Decreasing"),
            TrendIndicator::Stable => write!(f, "Stable"),
            TrendIndicator::Volatile => write!(f, "Volatile"),
        }
    }
}

/// Result of statistical analysis for a single value/invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalScore {
    /// Z-score (standard deviations from mean)
    pub z_score: f64,
    /// Percentile ranking (0-100)
    pub percentile: f64,
    /// Trend indicator based on recent values
    pub trend: TrendIndicator,
    /// Whether this value is considered an outlier
    pub is_outlier: bool,
    /// Baseline mean used for comparison
    pub baseline_mean: f64,
    /// Baseline standard deviation used for comparison
    pub baseline_stddev: f64,
    /// Number of samples in the baseline
    pub sample_count: u64,
}

impl StatisticalScore {
    /// Create a new statistical score
    pub fn new(
        value: f64,
        baseline: &BaselineStats,
        recent_values: &[f64],
        threshold: f64,
    ) -> Self {
        let z_score = baseline.z_score(value);
        let percentile = baseline.percentile_of(value);
        let trend = TrendIndicator::from_values(recent_values);
        let is_outlier = z_score.abs() > threshold;

        Self {
            z_score,
            percentile,
            trend,
            is_outlier,
            baseline_mean: baseline.mean,
            baseline_stddev: baseline.stddev,
            sample_count: baseline.count,
        }
    }
}

/// Time period for baseline computation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BaselinePeriod {
    /// Rolling 30-day window
    Rolling30Days,
    /// Rolling 90-day window
    Rolling90Days,
    /// Rolling 365-day window
    Rolling365Days,
    /// Specific month of year (1-12) for seasonal patterns
    MonthOfYear(u8),
}

impl BaselinePeriod {
    /// Get the number of days for this period (for rolling windows)
    pub fn days(&self) -> Option<u32> {
        match self {
            BaselinePeriod::Rolling30Days => Some(30),
            BaselinePeriod::Rolling90Days => Some(90),
            BaselinePeriod::Rolling365Days => Some(365),
            BaselinePeriod::MonthOfYear(_) => None,
        }
    }

    /// Create a cache key for this period
    pub fn cache_key(&self) -> String {
        match self {
            BaselinePeriod::Rolling30Days => "r30".to_string(),
            BaselinePeriod::Rolling90Days => "r90".to_string(),
            BaselinePeriod::Rolling365Days => "r365".to_string(),
            BaselinePeriod::MonthOfYear(m) => format!("m{}", m),
        }
    }
}

impl std::fmt::Display for BaselinePeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaselinePeriod::Rolling30Days => write!(f, "30 days"),
            BaselinePeriod::Rolling90Days => write!(f, "90 days"),
            BaselinePeriod::Rolling365Days => write!(f, "365 days"),
            BaselinePeriod::MonthOfYear(m) => write!(f, "month {}", m),
        }
    }
}

/// A computed baseline for a vendor/metric/period combination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorBaseline {
    /// Unique baseline ID
    pub id: BaselineId,
    /// Vendor name (grouping key)
    pub vendor_name: String,
    /// Metric name (e.g., "total_amount")
    pub metric_name: String,
    /// Time period for this baseline
    pub period: BaselinePeriod,
    /// Computed statistics
    pub stats: BaselineStats,
    /// Recent values for trend detection (newest last)
    pub recent_values: Vec<f64>,
    /// When this baseline was computed
    pub computed_at: DateTime<Utc>,
    /// When this baseline expires (should be recomputed)
    pub expires_at: DateTime<Utc>,
}

impl VendorBaseline {
    /// Check if this baseline has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Analyze a value against this baseline
    pub fn analyze(&self, value: f64, threshold: f64) -> StatisticalScore {
        StatisticalScore::new(value, &self.stats, &self.recent_values, threshold)
    }

    /// Create a cache key for this baseline
    pub fn cache_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.vendor_name,
            self.metric_name,
            self.period.cache_key()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_stats_from_values() {
        let values = vec![100.0, 200.0, 300.0, 400.0, 500.0];
        let stats = BaselineStats::from_values(&values);

        assert_eq!(stats.count, 5);
        assert!((stats.mean - 300.0).abs() < 0.01);
        assert_eq!(stats.min, 100.0);
        assert_eq!(stats.max, 500.0);
    }

    #[test]
    fn test_z_score() {
        let stats = BaselineStats {
            count: 100,
            mean: 1000.0,
            stddev: 100.0,
            min: 700.0,
            max: 1300.0,
            p25: 933.0,
            p50: 1000.0,
            p75: 1067.0,
            p90: 1128.0,
            p95: 1165.0,
            p99: 1233.0,
        };

        // Value at mean should have z-score of 0
        assert!((stats.z_score(1000.0)).abs() < 0.01);

        // Value 2 stddev above mean
        assert!((stats.z_score(1200.0) - 2.0).abs() < 0.01);

        // Value 2 stddev below mean
        assert!((stats.z_score(800.0) - (-2.0)).abs() < 0.01);
    }

    #[test]
    fn test_trend_detection() {
        // Increasing trend
        let increasing = vec![100.0, 150.0, 200.0, 250.0, 300.0];
        assert_eq!(
            TrendIndicator::from_values(&increasing),
            TrendIndicator::Increasing
        );

        // Decreasing trend
        let decreasing = vec![300.0, 250.0, 200.0, 150.0, 100.0];
        assert_eq!(
            TrendIndicator::from_values(&decreasing),
            TrendIndicator::Decreasing
        );

        // Stable trend
        let stable = vec![100.0, 102.0, 98.0, 101.0, 99.0];
        assert_eq!(TrendIndicator::from_values(&stable), TrendIndicator::Stable);
    }

    #[test]
    fn test_statistical_score() {
        let stats = BaselineStats {
            count: 50,
            mean: 1000.0,
            stddev: 200.0,
            min: 500.0,
            max: 1500.0,
            p25: 866.0,
            p50: 1000.0,
            p75: 1134.0,
            p90: 1256.0,
            p95: 1329.0,
            p99: 1465.0,
        };
        let recent = vec![950.0, 1000.0, 1050.0, 1000.0, 1020.0];

        // Normal value (within threshold)
        let score = StatisticalScore::new(1100.0, &stats, &recent, 2.0);
        assert!(!score.is_outlier);
        assert!((score.z_score - 0.5).abs() < 0.01);

        // Outlier (beyond threshold)
        let score = StatisticalScore::new(1500.0, &stats, &recent, 2.0);
        assert!(score.is_outlier);
        assert!((score.z_score - 2.5).abs() < 0.01);
    }
}
