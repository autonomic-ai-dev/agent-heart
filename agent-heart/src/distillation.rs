//! Weekly HDBSCAN-style cluster pulse on brain embeddings (Kernel V2 Phase 6).

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::brain_client::BrainHandle;

/// Default cosine-similarity threshold for merging clusters.
pub const DEFAULT_CLUSTER_THRESHOLD: f64 = 0.75;
/// Minimum cluster size (HDBSCAN min_samples analogue).
pub const MIN_CLUSTER_SIZE: usize = 2;
/// Weekly pulse: Sunday 02:00 UTC.
pub const WEEKLY_CRON: &str = "0 0 2 * * 0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterPulseReport {
    pub clusters_found: u64,
    pub items_clustered: u64,
    pub facts_created: u64,
    pub items_superseded: u64,
    pub threshold: f64,
    pub dry_run: bool,
}

/// Invoke agent-brain cluster distillation (embedding pulse).
pub async fn run_cluster_pulse(
    brain: &BrainHandle,
    threshold: f64,
    dry_run: bool,
) -> Result<ClusterPulseReport> {
    let stats = brain.call_distill(threshold, dry_run).await?;
    Ok(ClusterPulseReport {
        clusters_found: stats
            .get("clusters_found")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        items_clustered: stats
            .get("items_clustered")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        facts_created: stats
            .get("facts_created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        items_superseded: stats
            .get("items_superseded")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        threshold,
        dry_run,
    })
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let x = f64::from(*x);
        let y = f64::from(*y);
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= 0.0 || nb <= 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

fn find(parent: &mut [usize], i: usize) -> usize {
    let mut x = i;
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent[rb] = ra;
    }
}

/// HDBSCAN-lite merge: group embeddings with cosine similarity >= threshold.
/// Returns `(input_count, cluster_count)` after merging.
pub fn hdbscan_merge_count(
    embeddings: &[Vec<f32>],
    min_cluster_size: usize,
    threshold: f64,
) -> (usize, usize) {
    let n = embeddings.len();
    if n < min_cluster_size {
        return (n, n);
    }

    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in (i + 1)..n {
            if cosine_similarity(&embeddings[i], &embeddings[j]) >= threshold {
                union(&mut parent, i, j);
            }
        }
    }

    let mut roots = std::collections::HashSet::new();
    for i in 0..n {
        roots.insert(find(&mut parent, i));
    }
    (n, roots.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_reduces_fact_count() {
        let embeddings = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.99, 0.01, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.01, 0.99, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let (before, after) = hdbscan_merge_count(&embeddings, MIN_CLUSTER_SIZE, 0.9);
        assert_eq!(before, 5);
        assert!(after < before, "expected merge to reduce cluster count");
        assert_eq!(after, 3);
    }
}
