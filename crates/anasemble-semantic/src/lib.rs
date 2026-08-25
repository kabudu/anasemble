//! Canonical semantic snapshots and deterministic diffs. Text diffs are not semantic diffs.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use anasemble_core::{
    BoundedText, Budget, CompatibilityClass, Digest, SnapshotId, TemporalCut, encode_canonical_cbor,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Snapshot schema version.
pub const SCHEMA_VERSION: u16 = 1;

/// Fail-closed snapshot or diff error.
#[derive(Debug, Error)]
pub enum SemanticError {
    #[error(transparent)]
    Core(#[from] anasemble_core::CoreError),
    #[error(transparent)]
    Id(#[from] anasemble_core::IdError),
    #[error("{0}")]
    Protocol(&'static str),
    #[error("budget exhausted")]
    Budget,
}

/// Canonical semantic state that can be digested.
pub trait CanonicalSemanticState {
    /// Digest of the canonical encoding.
    fn digest(&self) -> Result<Digest, SemanticError>;
}

/// Produce a snapshot under a temporal cut and budget.
pub trait SemanticSnapshotter<I> {
    /// Snapshot type.
    type Snapshot: CanonicalSemanticState;
    /// Snapshot `input` at `cut`. Exhaustion is not success.
    fn snapshot(
        &self,
        input: &I,
        cut: TemporalCut,
        budget: Budget,
    ) -> Result<Self::Snapshot, SemanticError>;
}

/// Diff policy. Moves are exact digest matches across different ids.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiffPolicy {
    /// Classify same-digest, different-id pairs as `Move`.
    pub detect_moves: bool,
}

impl Default for DiffPolicy {
    fn default() -> Self {
        Self { detect_moves: true }
    }
}

/// Compare two snapshots.
pub trait SemanticDiffer<S> {
    /// Deterministic operations from `before` to `after`.
    fn diff(
        &self,
        before: &S,
        after: &S,
        policy: &DiffPolicy,
    ) -> Result<SemanticDiff, SemanticError>;
}

/// Node in a semantic graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticNode {
    /// Stable entity id.
    pub id: BoundedText,
    /// Entity kind.
    pub kind: BoundedText,
    /// Payload digest.
    pub digest: Digest,
}

/// Directed labelled edge.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticEdge {
    /// Source entity.
    pub from: BoundedText,
    /// Target entity.
    pub to: BoundedText,
    /// Edge kind.
    pub kind: BoundedText,
}

/// Completeness of a snapshot under the caller budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    /// Every admitted member is present.
    Complete,
    /// Budget truncated the snapshot.
    Truncated,
}

/// Canonical snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticSnapshotV1 {
    /// Schema version.
    pub schema_version: u16,
    /// Snapshot identity.
    pub snapshot_id: SnapshotId,
    /// Ontology version.
    pub ontology_version: BoundedText,
    /// Subject identity.
    pub subject: BoundedText,
    /// Valid and system cut.
    pub cut: TemporalCut,
    /// Nodes sorted by id.
    pub nodes: Vec<SemanticNode>,
    /// Edges sorted by (from, to, kind).
    pub edges: Vec<SemanticEdge>,
    /// Contract digests.
    pub contracts: Vec<Digest>,
    /// Evidence roots.
    pub evidence_roots: Vec<Digest>,
    /// Completeness claim.
    pub completeness: Completeness,
    /// Unknowns, sorted.
    pub unknowns: Vec<BoundedText>,
    /// Contradictions, sorted.
    pub contradictions: Vec<BoundedText>,
    /// Canonical digest.
    pub digest: Digest,
}

#[derive(Serialize)]
struct UnsignedSnapshot<'a> {
    schema_version: u16,
    ontology_version: &'a BoundedText,
    subject: &'a BoundedText,
    cut: &'a TemporalCut,
    nodes: &'a [SemanticNode],
    edges: &'a [SemanticEdge],
    contracts: &'a [Digest],
    evidence_roots: &'a [Digest],
    completeness: Completeness,
    unknowns: &'a [BoundedText],
    contradictions: &'a [BoundedText],
}

impl SemanticSnapshotV1 {
    /// Canonical digest of the unsigned snapshot.
    pub fn compute_digest(&self) -> Result<Digest, SemanticError> {
        Ok(Digest::sha256(&encode_canonical_cbor(&UnsignedSnapshot {
            schema_version: self.schema_version,
            ontology_version: &self.ontology_version,
            subject: &self.subject,
            cut: &self.cut,
            nodes: &self.nodes,
            edges: &self.edges,
            contracts: &self.contracts,
            evidence_roots: &self.evidence_roots,
            completeness: self.completeness,
            unknowns: &self.unknowns,
            contradictions: &self.contradictions,
        })?))
    }
}

impl CanonicalSemanticState for SemanticSnapshotV1 {
    fn digest(&self) -> Result<Digest, SemanticError> {
        self.compute_digest()
    }
}

/// Input graph for [`GraphSnapshotter`].
#[derive(Clone, Debug, Default)]
pub struct SemanticGraph {
    /// id -> (kind, payload digest)
    pub nodes: BTreeMap<String, (String, Digest)>,
    /// (from, to, kind)
    pub edges: Vec<(String, String, String)>,
    /// Contract digests.
    pub contracts: Vec<Digest>,
    /// Evidence roots.
    pub evidence_roots: Vec<Digest>,
    /// Unknown members.
    pub unknowns: Vec<String>,
    /// Contradictory members.
    pub contradictions: Vec<String>,
}

/// Snapshotter over a labelled graph.
#[derive(Clone, Debug)]
pub struct GraphSnapshotter {
    /// Ontology version bound into every snapshot.
    pub ontology_version: BoundedText,
    /// Subject identity.
    pub subject: BoundedText,
}

impl SemanticSnapshotter<SemanticGraph> for GraphSnapshotter {
    type Snapshot = SemanticSnapshotV1;

    fn snapshot(
        &self,
        input: &SemanticGraph,
        cut: TemporalCut,
        budget: Budget,
    ) -> Result<Self::Snapshot, SemanticError> {
        if cut.valid_at.is_empty() || cut.system_as_of.is_empty() {
            return Err(SemanticError::Protocol("temporal cut is incomplete"));
        }
        let node_count = u32::try_from(input.nodes.len()).unwrap_or(u32::MAX);
        let edge_count = u32::try_from(input.edges.len()).unwrap_or(u32::MAX);
        if budget.would_exceed(node_count, edge_count, 1, 1) {
            return Err(SemanticError::Budget);
        }
        let mut nodes: Vec<SemanticNode> = input
            .nodes
            .iter()
            .map(|(id, (kind, digest))| {
                Ok(SemanticNode {
                    id: BoundedText::new(id.clone())?,
                    kind: BoundedText::new(kind.clone())?,
                    digest: *digest,
                })
            })
            .collect::<Result<_, SemanticError>>()?;
        nodes.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        let mut edges: Vec<SemanticEdge> = input
            .edges
            .iter()
            .map(|(from, to, kind)| {
                Ok(SemanticEdge {
                    from: BoundedText::new(from.clone())?,
                    to: BoundedText::new(to.clone())?,
                    kind: BoundedText::new(kind.clone())?,
                })
            })
            .collect::<Result<_, SemanticError>>()?;
        edges.sort_by(|a, b| {
            (a.from.as_str(), a.to.as_str(), a.kind.as_str()).cmp(&(
                b.from.as_str(),
                b.to.as_str(),
                b.kind.as_str(),
            ))
        });
        let mut unknowns: Vec<BoundedText> = input
            .unknowns
            .iter()
            .map(|value| BoundedText::new(value.clone()).map_err(SemanticError::from))
            .collect::<Result<_, _>>()?;
        unknowns.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let mut contradictions: Vec<BoundedText> = input
            .contradictions
            .iter()
            .map(|value| BoundedText::new(value.clone()).map_err(SemanticError::from))
            .collect::<Result<_, _>>()?;
        contradictions.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let mut contracts = input.contracts.clone();
        contracts.sort_by_key(|a| a.to_hex());
        let mut evidence_roots = input.evidence_roots.clone();
        evidence_roots.sort_by_key(|a| a.to_hex());
        let mut snapshot = SemanticSnapshotV1 {
            schema_version: SCHEMA_VERSION,
            snapshot_id: SnapshotId::generate(),
            ontology_version: self.ontology_version.clone(),
            subject: self.subject.clone(),
            cut,
            nodes,
            edges,
            contracts,
            evidence_roots,
            completeness: Completeness::Complete,
            unknowns,
            contradictions,
            digest: Digest::sha256(b""),
        };
        snapshot.digest = snapshot.compute_digest()?;
        Ok(snapshot)
    }
}

/// Diff operation kind. Not a text edit script.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffOpKind {
    /// Entity appeared.
    Add,
    /// Entity disappeared.
    Remove,
    /// Same id, different digest.
    Modify,
    /// Same digest, different id.
    Move,
    /// Identity rebound.
    Rebind,
    /// One entity became many.
    Split,
    /// Many entities became one.
    Merge,
    /// Contract digest changed.
    ContractChange,
    /// Classification exhausted the policy.
    Unknown,
}

/// One typed operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiffOperation {
    /// Operation kind.
    pub kind: DiffOpKind,
    /// Primary entity id.
    pub entity_id: BoundedText,
    /// Before digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Digest>,
    /// After digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Digest>,
    /// Compatibility class.
    pub compatibility: CompatibilityClass,
    /// Evidence roots for the operation.
    pub evidence: Vec<Digest>,
}

/// Deterministic semantic diff.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticDiff {
    /// Schema version.
    pub schema_version: u16,
    /// Diff identity.
    pub diff_id: anasemble_core::DiffId,
    /// Before snapshot digest.
    pub before_digest: Digest,
    /// After snapshot digest.
    pub after_digest: Digest,
    /// Sorted operations.
    pub operations: Vec<DiffOperation>,
    /// Canonical digest.
    pub digest: Digest,
}

#[derive(Serialize)]
struct UnsignedDiff<'a> {
    schema_version: u16,
    before_digest: Digest,
    after_digest: Digest,
    operations: &'a [DiffOperation],
}

impl SemanticDiff {
    fn bind(mut self) -> Result<Self, SemanticError> {
        self.digest = Digest::sha256(&encode_canonical_cbor(&UnsignedDiff {
            schema_version: self.schema_version,
            before_digest: self.before_digest,
            after_digest: self.after_digest,
            operations: &self.operations,
        })?);
        Ok(self)
    }
}

/// Canonical differ. Matching is by stable entity id, then exact digest moves.
#[derive(Clone, Debug, Default)]
pub struct CanonicalDiffer;

impl SemanticDiffer<SemanticSnapshotV1> for CanonicalDiffer {
    fn diff(
        &self,
        before: &SemanticSnapshotV1,
        after: &SemanticSnapshotV1,
        policy: &DiffPolicy,
    ) -> Result<SemanticDiff, SemanticError> {
        if before.compute_digest()? != before.digest || after.compute_digest()? != after.digest {
            return Err(SemanticError::Protocol("snapshot digest mismatch"));
        }
        let before_nodes: BTreeMap<&str, &SemanticNode> = before
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let after_nodes: BTreeMap<&str, &SemanticNode> = after
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let mut operations = Vec::new();
        for (id, node) in &before_nodes {
            match after_nodes.get(id) {
                None => operations.push(DiffOperation {
                    kind: DiffOpKind::Remove,
                    entity_id: node.id.clone(),
                    before: Some(node.digest),
                    after: None,
                    compatibility: CompatibilityClass::Unknown,
                    evidence: after.evidence_roots.clone(),
                }),
                Some(next) if next.digest != node.digest => operations.push(DiffOperation {
                    kind: DiffOpKind::Modify,
                    entity_id: node.id.clone(),
                    before: Some(node.digest),
                    after: Some(next.digest),
                    compatibility: CompatibilityClass::Unknown,
                    evidence: after.evidence_roots.clone(),
                }),
                Some(_) => {}
            }
        }
        for (id, node) in &after_nodes {
            if !before_nodes.contains_key(id) {
                operations.push(DiffOperation {
                    kind: DiffOpKind::Add,
                    entity_id: node.id.clone(),
                    before: None,
                    after: Some(node.digest),
                    compatibility: CompatibilityClass::Unknown,
                    evidence: after.evidence_roots.clone(),
                });
            }
        }
        if policy.detect_moves {
            classify_moves(&mut operations);
        }
        if before.contracts != after.contracts {
            operations.push(DiffOperation {
                kind: DiffOpKind::ContractChange,
                entity_id: BoundedText::new("contracts")?,
                before: before.contracts.first().copied(),
                after: after.contracts.first().copied(),
                compatibility: CompatibilityClass::Unknown,
                evidence: after.evidence_roots.clone(),
            });
        }
        operations.sort_by(|a, b| {
            (op_rank(a.kind), a.entity_id.as_str()).cmp(&(op_rank(b.kind), b.entity_id.as_str()))
        });
        SemanticDiff {
            schema_version: SCHEMA_VERSION,
            diff_id: anasemble_core::DiffId::generate(),
            before_digest: before.digest,
            after_digest: after.digest,
            operations,
            digest: Digest::sha256(b""),
        }
        .bind()
    }
}

fn op_rank(kind: DiffOpKind) -> u8 {
    match kind {
        DiffOpKind::Remove => 0,
        DiffOpKind::Add => 1,
        DiffOpKind::Modify => 2,
        DiffOpKind::Move => 3,
        DiffOpKind::Rebind => 4,
        DiffOpKind::Split => 5,
        DiffOpKind::Merge => 6,
        DiffOpKind::ContractChange => 7,
        DiffOpKind::Unknown => 8,
    }
}

fn classify_moves(operations: &mut Vec<DiffOperation>) {
    let mut used_adds = std::collections::BTreeSet::new();
    let mut drop_set = std::collections::BTreeSet::new();
    let mut moves = Vec::new();
    for (remove_idx, op) in operations.iter().enumerate() {
        if op.kind != DiffOpKind::Remove {
            continue;
        }
        let Some(digest) = op.before else {
            continue;
        };
        let Some((add_idx, add)) = operations.iter().enumerate().find(|(idx, add)| {
            add.kind == DiffOpKind::Add && add.after == Some(digest) && !used_adds.contains(idx)
        }) else {
            continue;
        };
        used_adds.insert(add_idx);
        drop_set.insert(remove_idx);
        drop_set.insert(add_idx);
        moves.push(DiffOperation {
            kind: DiffOpKind::Move,
            entity_id: add.entity_id.clone(),
            before: op.before,
            after: add.after,
            compatibility: CompatibilityClass::Backward,
            evidence: add.evidence.clone(),
        });
    }
    if moves.is_empty() {
        return;
    }
    let mut kept: Vec<DiffOperation> = operations
        .iter()
        .enumerate()
        .filter(|(idx, _)| !drop_set.contains(idx))
        .map(|(_, op)| op.clone())
        .collect();
    kept.extend(moves);
    *operations = kept;
}

/// Independent snapshot digest using a BTree rebuild.
pub fn independent_snapshot_digest(snapshot: &SemanticSnapshotV1) -> Result<Digest, SemanticError> {
    let nodes: BTreeMap<_, _> = snapshot
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let rebuilt: Vec<SemanticNode> = nodes.into_values().cloned().collect();
    let view = UnsignedSnapshot {
        schema_version: snapshot.schema_version,
        ontology_version: &snapshot.ontology_version,
        subject: &snapshot.subject,
        cut: &snapshot.cut,
        nodes: &rebuilt,
        edges: &snapshot.edges,
        contracts: &snapshot.contracts,
        evidence_roots: &snapshot.evidence_roots,
        completeness: snapshot.completeness,
        unknowns: &snapshot.unknowns,
        contradictions: &snapshot.contradictions,
    };
    Ok(Digest::sha256(&encode_canonical_cbor(&view)?))
}

/// Independent diff digest.
pub fn independent_diff_digest(diff: &SemanticDiff) -> Result<Digest, SemanticError> {
    Ok(Digest::sha256(&encode_canonical_cbor(&UnsignedDiff {
        schema_version: diff.schema_version,
        before_digest: diff.before_digest,
        after_digest: diff.after_digest,
        operations: &diff.operations,
    })?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapper() -> GraphSnapshotter {
        GraphSnapshotter {
            ontology_version: BoundedText::new("onto-1").unwrap(),
            subject: BoundedText::new("system:demo").unwrap(),
        }
    }

    fn cut() -> TemporalCut {
        TemporalCut::new("2026-08-25T00:00:00Z", "2026-08-25T00:00:01Z")
    }

    #[test]
    fn identical_inputs_have_identical_digests_except_id() {
        let mut graph = SemanticGraph::default();
        graph
            .nodes
            .insert("a".into(), ("service".into(), Digest::sha256(b"svc")));
        let first = snapper()
            .snapshot(&graph, cut(), Budget::standard())
            .unwrap();
        let second = snapper()
            .snapshot(&graph, cut(), Budget::standard())
            .unwrap();
        assert_eq!(independent_snapshot_digest(&first).unwrap(), first.digest);
        assert_eq!(first.digest, second.digest);
        assert_ne!(first.snapshot_id, second.snapshot_id);
    }

    #[test]
    fn diff_classifies_add_remove_modify_and_is_stable() {
        let mut before_g = SemanticGraph::default();
        before_g
            .nodes
            .insert("keep".into(), ("svc".into(), Digest::sha256(b"k")));
        before_g
            .nodes
            .insert("gone".into(), ("svc".into(), Digest::sha256(b"g")));
        before_g
            .nodes
            .insert("chg".into(), ("svc".into(), Digest::sha256(b"old")));
        let mut after_g = SemanticGraph::default();
        after_g
            .nodes
            .insert("keep".into(), ("svc".into(), Digest::sha256(b"k")));
        after_g
            .nodes
            .insert("new".into(), ("svc".into(), Digest::sha256(b"n")));
        after_g
            .nodes
            .insert("chg".into(), ("svc".into(), Digest::sha256(b"new")));
        let before = snapper()
            .snapshot(&before_g, cut(), Budget::standard())
            .unwrap();
        let after = snapper()
            .snapshot(&after_g, cut(), Budget::standard())
            .unwrap();
        let diff = CanonicalDiffer
            .diff(&before, &after, &DiffPolicy::default())
            .unwrap();
        let kinds: Vec<_> = diff.operations.iter().map(|op| op.kind).collect();
        assert!(kinds.contains(&DiffOpKind::Add));
        assert!(kinds.contains(&DiffOpKind::Remove));
        assert!(kinds.contains(&DiffOpKind::Modify));
        let again = CanonicalDiffer
            .diff(&before, &after, &DiffPolicy::default())
            .unwrap();
        assert_eq!(
            diff.operations
                .iter()
                .map(|op| (op.kind, op.entity_id.as_str().to_owned()))
                .collect::<Vec<_>>(),
            again
                .operations
                .iter()
                .map(|op| (op.kind, op.entity_id.as_str().to_owned()))
                .collect::<Vec<_>>()
        );
        assert_eq!(independent_diff_digest(&diff).unwrap(), diff.digest);
    }

    #[test]
    fn same_digest_different_id_is_a_move() {
        let mut before_g = SemanticGraph::default();
        before_g
            .nodes
            .insert("old".into(), ("svc".into(), Digest::sha256(b"same")));
        let mut after_g = SemanticGraph::default();
        after_g
            .nodes
            .insert("new".into(), ("svc".into(), Digest::sha256(b"same")));
        let before = snapper()
            .snapshot(&before_g, cut(), Budget::standard())
            .unwrap();
        let after = snapper()
            .snapshot(&after_g, cut(), Budget::standard())
            .unwrap();
        let diff = CanonicalDiffer
            .diff(&before, &after, &DiffPolicy::default())
            .unwrap();
        assert_eq!(diff.operations.len(), 1);
        assert_eq!(diff.operations[0].kind, DiffOpKind::Move);
        assert_eq!(diff.operations[0].entity_id.as_str(), "new");
    }

    #[test]
    fn budget_exhaustion_is_not_a_snapshot() {
        let mut graph = SemanticGraph::default();
        graph
            .nodes
            .insert("a".into(), ("svc".into(), Digest::sha256(b"a")));
        graph
            .nodes
            .insert("b".into(), ("svc".into(), Digest::sha256(b"b")));
        let tiny = Budget::new(1, 1, 1, 8, 10).unwrap();
        assert!(matches!(
            snapper().snapshot(&graph, cut(), tiny),
            Err(SemanticError::Budget)
        ));
    }
}
