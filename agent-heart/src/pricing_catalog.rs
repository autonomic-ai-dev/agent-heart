use crate::usage_schema::BillingType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenUsageCounts {
    pub prompt: u64,
    pub completion: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenUsageCounts {
    pub fn new(prompt: u64, completion: u64, cache_read: u64, cache_write: u64) -> Self {
        Self {
            prompt,
            completion,
            cache_read,
            cache_write,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelPricing {
    pub prompt_per_million: f64,
    pub completion_per_million: f64,
    pub cache_read_per_million: f64,
    pub cache_write_per_million: f64,
}

impl ModelPricing {
    pub fn new(prompt: f64, completion: f64, cache_read: f64, cache_write: f64) -> Self {
        Self {
            prompt_per_million: prompt,
            completion_per_million: completion,
            cache_read_per_million: cache_read,
            cache_write_per_million: cache_write,
        }
    }

    pub fn calculate_cost(&self, tokens: TokenUsageCounts) -> f64 {
        let p_cost = (tokens.prompt as f64 / 1_000_000.0) * self.prompt_per_million;
        let c_cost = (tokens.completion as f64 / 1_000_000.0) * self.completion_per_million;
        let cr_cost = (tokens.cache_read as f64 / 1_000_000.0) * self.cache_read_per_million;
        let cw_cost = (tokens.cache_write as f64 / 1_000_000.0) * self.cache_write_per_million;
        p_cost + c_cost + cr_cost + cw_cost
    }
}

pub struct PricingCatalog {
    entries: HashMap<String, ModelPricing>,
}

impl Default for PricingCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl PricingCatalog {
    pub fn new() -> Self {
        let mut entries = HashMap::new();

        // Anthropic models
        entries.insert(
            "claude-3-7-sonnet".to_string(),
            ModelPricing::new(3.00, 15.00, 0.30, 3.75),
        );
        entries.insert(
            "claude-3-5-sonnet".to_string(),
            ModelPricing::new(3.00, 15.00, 0.30, 3.75),
        );
        entries.insert(
            "claude-3-5-haiku".to_string(),
            ModelPricing::new(0.80, 4.00, 0.08, 1.00),
        );
        entries.insert(
            "claude-3-opus".to_string(),
            ModelPricing::new(15.00, 75.00, 1.50, 18.75),
        );

        // OpenAI models
        entries.insert(
            "gpt-4o".to_string(),
            ModelPricing::new(2.50, 10.00, 1.25, 2.50),
        );
        entries.insert(
            "gpt-4o-mini".to_string(),
            ModelPricing::new(0.15, 0.60, 0.075, 0.15),
        );
        entries.insert(
            "o1".to_string(),
            ModelPricing::new(15.00, 60.00, 7.50, 15.00),
        );
        entries.insert(
            "o3-mini".to_string(),
            ModelPricing::new(1.10, 4.40, 0.55, 1.10),
        );

        // Google Gemini models
        entries.insert(
            "gemini-2.0-flash".to_string(),
            ModelPricing::new(0.10, 0.40, 0.025, 0.10),
        );
        entries.insert(
            "gemini-1.5-pro".to_string(),
            ModelPricing::new(1.25, 5.00, 0.3125, 1.25),
        );
        entries.insert(
            "gemini-1.5-flash".to_string(),
            ModelPricing::new(0.075, 0.30, 0.01875, 0.075),
        );

        // DeepSeek models
        entries.insert(
            "deepseek-chat".to_string(),
            ModelPricing::new(0.14, 0.28, 0.014, 0.14),
        );
        entries.insert(
            "deepseek-reasoner".to_string(),
            ModelPricing::new(0.55, 2.19, 0.14, 0.55),
        );

        Self { entries }
    }

    pub fn get_pricing(&self, model: &str) -> Option<&ModelPricing> {
        let clean = model.to_ascii_lowercase();
        // Exact match
        if let Some(p) = self.entries.get(&clean) {
            return Some(p);
        }
        // Substring / prefix match
        for (k, v) in &self.entries {
            if clean.contains(k) || k.contains(&clean) {
                return Some(v);
            }
        }
        None
    }

    pub fn calculate_cost(
        &self,
        provider: &str,
        model: &str,
        billing_type: BillingType,
        tokens: TokenUsageCounts,
    ) -> f64 {
        // Free tiers have zero billed dollar cost
        if billing_type == BillingType::LocalFree || billing_type == BillingType::SubscriptionQuota
        {
            return 0.0;
        }

        if provider.eq_ignore_ascii_case("local") || provider.eq_ignore_ascii_case("ollama") {
            return 0.0;
        }

        match self.get_pricing(model) {
            Some(pricing) => pricing.calculate_cost(tokens),
            None => {
                // Default fallback rate: $1.00 / 1M prompt, $3.00 / 1M completion
                let fallback = ModelPricing::new(1.00, 3.00, 0.10, 1.00);
                fallback.calculate_cost(tokens)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_sonnet_cost() {
        // Arrange
        let catalog = PricingCatalog::new();
        let tokens = TokenUsageCounts::new(1_000_000, 100_000, 500_000, 0);

        // Act
        let cost = catalog.calculate_cost(
            "anthropic",
            "claude-3-7-sonnet",
            BillingType::MeteredApi,
            tokens,
        );

        // Assert:
        // Prompt: 1M * 3.00 = 3.00
        // Completion: 0.1M * 15.00 = 1.50
        // Cache read: 0.5M * 0.30 = 0.15
        // Total = 4.65
        assert!((cost - 4.65).abs() < 1e-4);
    }

    #[test]
    fn test_subscription_and_local_are_zero_cost() {
        // Arrange
        let catalog = PricingCatalog::new();
        let tokens = TokenUsageCounts::new(1_000_000, 1_000_000, 0, 0);

        // Act
        let sub_cost = catalog.calculate_cost(
            "anthropic",
            "claude-3-7-sonnet",
            BillingType::SubscriptionQuota,
            tokens,
        );
        let local_cost =
            catalog.calculate_cost("local", "deepseek-r1:8b", BillingType::LocalFree, tokens);

        // Assert
        assert_eq!(sub_cost, 0.0);
        assert_eq!(local_cost, 0.0);
    }
}
