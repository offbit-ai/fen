//! Statistical anomaly detection for invoice validation.
//!
//! This module provides statistical analysis capabilities for detecting
//! invoices that deviate significantly from vendor-specific baselines.

use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use fen_core::domain::{
    Anomaly, AnomalyType, BaselineStats, Invoice, Severity, StatisticalScore, VendorBaseline,
};

/// Configuration for statistical analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalAnalyzerConfig {
    /// Enable statistical analysis
    pub enabled: bool,
    /// Default z-score threshold for outlier detection (default: 2.0)
    pub threshold: f64,
    /// Minimum samples required for baseline analysis (default: 5)
    pub min_samples: u64,
    /// Metrics to analyze (default: ["total_amount"])
    pub metrics: Vec<String>,
    /// Severity for statistical outliers (default: Medium)
    pub outlier_severity: Severity,
}

impl Default for StatisticalAnalyzerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 2.0,
            min_samples: 5,
            metrics: vec!["total_amount".to_string()],
            outlier_severity: Severity::Medium,
        }
    }
}

/// Statistical analyzer for invoice anomaly detection
#[derive(Debug, Clone)]
pub struct StatisticalAnalyzer {
    config: StatisticalAnalyzerConfig,
}

impl StatisticalAnalyzer {
    /// Create a new statistical analyzer with default configuration
    pub fn new() -> Self {
        Self {
            config: StatisticalAnalyzerConfig::default(),
        }
    }

    /// Create a new statistical analyzer with custom configuration
    pub fn with_config(config: StatisticalAnalyzerConfig) -> Self {
        Self { config }
    }

    /// Check if statistical analysis is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the configured threshold
    pub fn threshold(&self) -> f64 {
        self.config.threshold
    }

    /// Get the configured metrics
    pub fn metrics(&self) -> &[String] {
        &self.config.metrics
    }

    /// Analyze an invoice against a vendor baseline
    pub fn analyze(
        &self,
        invoice: &Invoice,
        baseline: &VendorBaseline,
    ) -> Option<StatisticalAnalysisResult> {
        if !self.config.enabled {
            return None;
        }

        // Check minimum sample count
        if baseline.stats.count < self.config.min_samples {
            tracing::debug!(
                vendor = %invoice.vendor.name,
                sample_count = baseline.stats.count,
                min_samples = self.config.min_samples,
                "Insufficient baseline samples for statistical analysis"
            );
            return None;
        }

        let value = self.extract_metric(invoice, &baseline.metric_name)?;
        let score = baseline.analyze(value, self.config.threshold);

        Some(StatisticalAnalysisResult {
            metric_name: baseline.metric_name.clone(),
            value,
            score,
        })
    }

    /// Analyze an invoice and generate anomalies if outliers are detected
    pub fn analyze_for_anomalies(
        &self,
        invoice: &Invoice,
        baselines: &[VendorBaseline],
    ) -> Vec<Anomaly> {
        if !self.config.enabled {
            return vec![];
        }

        let mut anomalies = Vec::new();

        for baseline in baselines {
            if let Some(result) = self.analyze(invoice, baseline) {
                if result.score.is_outlier {
                    let anomaly = self.create_outlier_anomaly(invoice, &result);
                    anomalies.push(anomaly);
                }
            }
        }

        anomalies
    }

    /// Create an anomaly for a detected statistical outlier
    fn create_outlier_anomaly(
        &self,
        invoice: &Invoice,
        result: &StatisticalAnalysisResult,
    ) -> Anomaly {
        let direction = if result.score.z_score > 0.0 {
            "above"
        } else {
            "below"
        };

        let description = format!(
            "{} (${:.2}) is {:.1} standard deviations {} vendor baseline (mean: ${:.2}, stddev: ${:.2})",
            format_metric_name(&result.metric_name),
            result.value,
            result.score.z_score.abs(),
            direction,
            result.score.baseline_mean,
            result.score.baseline_stddev
        );

        Anomaly::new(
            invoice.document_id,
            AnomalyType::StatisticalOutlier,
            self.config.outlier_severity,
            description,
        )
        .with_field(&result.metric_name)
        .with_values(
            format!("{:.2}", result.score.baseline_mean),
            format!("{:.2}", result.value),
        )
        .with_statistical_score(result.score.clone())
        .with_detected_at(Utc::now())
    }

    /// Extract a metric value from an invoice
    pub fn extract_metric(&self, invoice: &Invoice, metric: &str) -> Option<f64> {
        match metric {
            "total_amount" => invoice.total_amount.to_f64(),
            "subtotal" => invoice.subtotal.to_f64(),
            "tax_amount" => invoice.tax_amount.to_f64(),
            "discount_amount" => invoice.discount_amount.to_f64(),
            "line_item_count" => Some(invoice.line_items.len() as f64),
            "line_items_sum" => invoice.line_items_sum().to_f64(),
            "confidence_score" => Some(invoice.confidence_score as f64),
            _ => {
                tracing::warn!(metric, "Unknown metric for extraction");
                None
            }
        }
    }

    /// Compute baseline statistics from a set of invoices
    pub fn compute_baseline(&self, invoices: &[Invoice], metric: &str) -> Option<BaselineStats> {
        let values: Vec<f64> = invoices
            .iter()
            .filter_map(|inv| self.extract_metric(inv, metric))
            .collect();

        if values.is_empty() {
            return None;
        }

        Some(BaselineStats::from_values(&values))
    }

    /// Extract recent values for trend analysis (ordered by invoice date, newest last)
    pub fn extract_recent_values(
        &self,
        invoices: &mut [Invoice],
        metric: &str,
        count: usize,
    ) -> Vec<f64> {
        // Sort by invoice date
        invoices.sort_by(|a, b| a.invoice_date.cmp(&b.invoice_date));

        invoices
            .iter()
            .rev()
            .take(count)
            .filter_map(|inv| self.extract_metric(inv, metric))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

impl Default for StatisticalAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of statistical analysis on a single metric
#[derive(Debug, Clone)]
pub struct StatisticalAnalysisResult {
    /// Name of the analyzed metric
    pub metric_name: String,
    /// Actual value from the invoice
    pub value: f64,
    /// Statistical score computed against baseline
    pub score: StatisticalScore,
}

/// Format a metric name for human display
fn format_metric_name(metric: &str) -> String {
    match metric {
        "total_amount" => "Invoice total".to_string(),
        "subtotal" => "Subtotal".to_string(),
        "tax_amount" => "Tax amount".to_string(),
        "discount_amount" => "Discount amount".to_string(),
        "line_item_count" => "Number of line items".to_string(),
        "line_items_sum" => "Line items sum".to_string(),
        "confidence_score" => "Extraction confidence".to_string(),
        other => other.replace('_', " ").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use fen_core::domain::{BaselineId, BaselinePeriod, TrendIndicator};
    use rust_decimal::Decimal;

    fn create_test_invoice(total: f64) -> Invoice {
        let mut invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        invoice.total_amount = Decimal::try_from(total).unwrap();
        invoice.subtotal = Decimal::try_from(total).unwrap();
        invoice.confidence_score = 0.95;
        invoice
    }

    fn create_test_baseline(mean: f64, stddev: f64) -> VendorBaseline {
        VendorBaseline {
            id: BaselineId::new(),
            vendor_name: "Test Vendor".to_string(),
            metric_name: "total_amount".to_string(),
            period: BaselinePeriod::Rolling90Days,
            stats: BaselineStats {
                count: 50,
                mean,
                stddev,
                min: mean - 3.0 * stddev,
                max: mean + 3.0 * stddev,
                p25: mean - 0.67 * stddev,
                p50: mean,
                p75: mean + 0.67 * stddev,
                p90: mean + 1.28 * stddev,
                p95: mean + 1.64 * stddev,
                p99: mean + 2.33 * stddev,
            },
            recent_values: vec![mean - 10.0, mean, mean + 10.0],
            computed_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
        }
    }

    #[test]
    fn test_analyze_normal_value() {
        let analyzer = StatisticalAnalyzer::new();
        let invoice = create_test_invoice(1050.0);
        let baseline = create_test_baseline(1000.0, 100.0);

        let result = analyzer.analyze(&invoice, &baseline);
        assert!(result.is_some());

        let result = result.unwrap();
        assert!(!result.score.is_outlier);
        assert!((result.score.z_score - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_analyze_outlier_value() {
        let analyzer = StatisticalAnalyzer::new();
        let invoice = create_test_invoice(1500.0); // 5 stddev above mean
        let baseline = create_test_baseline(1000.0, 100.0);

        let result = analyzer.analyze(&invoice, &baseline);
        assert!(result.is_some());

        let result = result.unwrap();
        assert!(result.score.is_outlier);
        assert!((result.score.z_score - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_analyze_for_anomalies() {
        let analyzer = StatisticalAnalyzer::new();
        let invoice = create_test_invoice(1500.0);
        let baseline = create_test_baseline(1000.0, 100.0);

        let anomalies = analyzer.analyze_for_anomalies(&invoice, &[baseline]);
        assert_eq!(anomalies.len(), 1);

        let anomaly = &anomalies[0];
        assert_eq!(anomaly.anomaly_type, AnomalyType::StatisticalOutlier);
        assert!(anomaly.statistical_score.is_some());
    }

    #[test]
    fn test_disabled_analyzer() {
        let config = StatisticalAnalyzerConfig {
            enabled: false,
            ..Default::default()
        };
        let analyzer = StatisticalAnalyzer::with_config(config);

        let invoice = create_test_invoice(1500.0);
        let baseline = create_test_baseline(1000.0, 100.0);

        let result = analyzer.analyze(&invoice, &baseline);
        assert!(result.is_none());

        let anomalies = analyzer.analyze_for_anomalies(&invoice, &[baseline]);
        assert!(anomalies.is_empty());
    }

    #[test]
    fn test_insufficient_samples() {
        let analyzer = StatisticalAnalyzer::new();
        let invoice = create_test_invoice(1500.0);

        let mut baseline = create_test_baseline(1000.0, 100.0);
        baseline.stats.count = 3; // Below min_samples (5)

        let result = analyzer.analyze(&invoice, &baseline);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_metrics() {
        let analyzer = StatisticalAnalyzer::new();
        let mut invoice = create_test_invoice(1000.0);
        invoice.tax_amount = Decimal::new(100, 0);
        invoice.discount_amount = Decimal::new(50, 0);

        assert_eq!(
            analyzer.extract_metric(&invoice, "total_amount"),
            Some(1000.0)
        );
        assert_eq!(analyzer.extract_metric(&invoice, "tax_amount"), Some(100.0));
        assert_eq!(
            analyzer.extract_metric(&invoice, "discount_amount"),
            Some(50.0)
        );
        assert_eq!(
            analyzer.extract_metric(&invoice, "line_item_count"),
            Some(0.0)
        );
        assert!(analyzer
            .extract_metric(&invoice, "unknown_metric")
            .is_none());
    }

    #[test]
    fn test_custom_threshold() {
        let config = StatisticalAnalyzerConfig {
            threshold: 4.0, // Higher threshold
            ..Default::default()
        };
        let analyzer = StatisticalAnalyzer::with_config(config);

        let invoice = create_test_invoice(1300.0); // 3 stddev above
        let baseline = create_test_baseline(1000.0, 100.0);

        let result = analyzer.analyze(&invoice, &baseline).unwrap();
        // At 3 stddev, should NOT be outlier with 4.0 threshold
        assert!(!result.score.is_outlier);
    }

    #[test]
    fn test_compute_baseline() {
        let analyzer = StatisticalAnalyzer::new();

        let invoices: Vec<Invoice> = vec![
            create_test_invoice(100.0),
            create_test_invoice(150.0),
            create_test_invoice(200.0),
            create_test_invoice(180.0),
            create_test_invoice(170.0),
        ];

        let stats = analyzer
            .compute_baseline(&invoices, "total_amount")
            .unwrap();

        assert_eq!(stats.count, 5);
        assert!((stats.mean - 160.0).abs() < 0.01);
        assert_eq!(stats.min, 100.0);
        assert_eq!(stats.max, 200.0);
    }
}
