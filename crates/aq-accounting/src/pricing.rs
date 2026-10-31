use crate::{Category, Error, Layout, Result, wire};
use serde::{Deserialize, Serialize};
use wire::{Cost, EstimateBasis, UnknownReason, Validate};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub category: Category,
    pub subtract: Vec<Category>,
    pub microunits_per_million_tokens: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateCard {
    pub reference: wire::ArtifactRef,
    pub allocation_policy: wire::ArtifactRef,
    pub currency: wire::Text,
    pub provider: wire::Text,
    pub model: wire::Text,
    pub layout: Layout,
    pub rules: Vec<Rule>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pricing {
    pub cost: Cost,
    /// Explicit non-invoice lower bound, never a complete budget assertion.
    pub known_lower_bound: Cost,
    pub missing_categories: Vec<Category>,
}
impl RateCard {
    pub fn validate(&self) -> Result<()> {
        wire::artifact(&self.reference)?;
        wire::artifact(&self.allocation_policy)?;
        self.layout.validate()?;
        self.currency.validate()?;
        self.provider.validate()?;
        self.model.validate()?;
        if self.currency.as_str().len() != 3
            || !self
                .currency
                .as_str()
                .bytes()
                .all(|b| b.is_ascii_uppercase())
            || self.rules.is_empty()
            || self.rules.len() > 5
        {
            return Err(Error::Invalid);
        }
        let mut covered = 0u8;
        for rule in &self.rules {
            if rule.subtract.len() > 3 {
                return Err(Error::Invalid);
            }
            let base = self.layout.atoms(rule.category);
            let mut subtract = 0u8;
            for c in &rule.subtract {
                let atoms = self.layout.atoms(*c);
                if atoms & base != atoms || atoms & subtract != 0 {
                    return Err(Error::Invalid);
                }
                subtract |= atoms;
            }
            let atoms = base & !subtract;
            if atoms == 0 || covered & atoms != 0 {
                return Err(Error::Invalid);
            }
            covered |= atoms;
        }
        // All five semantic atoms must be priced explicitly, including a zero
        // rate where appropriate. This prevents an omitted category looking free.
        if covered != 31 {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn estimate(
        &self,
        provider: &wire::Text,
        model: &wire::Text,
        counters: &wire::Counters,
    ) -> Result<Pricing> {
        self.validate()?;
        self.layout.check_counters(counters)?;
        if provider != &self.provider || model != &self.model {
            return Err(Error::Unsupported);
        }
        let mut weighted = 0u128;
        let mut missing = std::collections::BTreeSet::new();
        for rule in &self.rules {
            if rule.microunits_per_million_tokens == 0 {
                continue;
            }
            let mut count = match rule.category.get(counters) {
                wire::Counter::Known { value } => Some(*value),
                wire::Counter::Unknown { .. } => {
                    missing.insert(rule.category);
                    None
                }
            };
            for c in &rule.subtract {
                match c.get(counters) {
                    wire::Counter::Known { value } => {
                        if let Some(v) = count {
                            count = Some(v.checked_sub(*value).ok_or(Error::Conflict)?);
                        }
                    }
                    wire::Counter::Unknown { .. } => {
                        missing.insert(*c);
                        count = None;
                    }
                }
            }
            if let Some(count) = count {
                weighted = weighted
                    .checked_add(u128::from(count) * u128::from(rule.microunits_per_million_tokens))
                    .ok_or(Error::Overflow)?;
            }
        }
        // Round once per canonical contribution, upwards to a microunit. All
        // calculations use integers; neither binary float nor currency guessing.
        let amount = weighted.checked_add(999_999).ok_or(Error::Overflow)? / 1_000_000;
        let amount_microunits = u64::try_from(amount).map_err(|_| Error::Overflow)?;
        let estimate = Cost::NonInvoiceEstimate {
            amount_microunits,
            currency: self.currency.clone(),
            rate_card: self.reference.clone(),
            basis: EstimateBasis::TokenRates,
            allocation_policy: self.allocation_policy.clone(),
        };
        Ok(Pricing {
            cost: if missing.is_empty() {
                estimate.clone()
            } else {
                Cost::Unknown {
                    reason: UnknownReason::Incomplete,
                }
            },
            known_lower_bound: estimate,
            missing_categories: missing.into_iter().collect(),
        })
    }
}

impl RateCard {
    /// Production entry: use the canonical fact's pinned card and verified
    /// category policy. Never price raw transcript fields using a different
    /// layout merely because the model display string matches.
    pub fn estimate_fact(&self, fact: &crate::reconcile::Fact) -> Result<Pricing> {
        fact.validate()?;
        if fact.pricing_reference.as_ref() != Some(&self.reference) || fact.layout != self.layout {
            return Err(Error::Unsupported);
        }
        self.estimate(
            &fact.usage.call.provider,
            &fact.usage.call.model,
            &fact.contribution,
        )
    }
}
