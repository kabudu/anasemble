//! A2-A4 traces, snapshots/diffs, contracts, and verification certificates.

use anasemble::{
    Budget, CanonicalDiffer, CompatibilityClass, ContractV1, DiffPolicy, Digest, DigestCandidate,
    DigestEqualityVerifier, GraphSnapshotter, Recorder, Replayer, SemanticDiffer, SemanticGraph,
    SemanticSnapshotter, SigningKey, TemporalCut, Verifier,
};
use anasemble_semantic::independent_snapshot_digest;
use anasemble_trace::{
    PURE_CAPABILITY, independent_trace_digest, partition_held_out, sign_trace, verify_trace,
};
use anasemble_verification::{independent_report_digest, sign_report, verify_certificate};

#[test]
fn historical_envelope_reexport_still_exists() {
    let _ = std::any::type_name::<anasemble::EvidenceEnvelopeV1>();
    let _ = std::any::type_name::<anasemble::EventLog>();
}

#[test]
fn trace_snapshot_diff_and_certificate_are_independently_checkable() {
    let budget = Budget::standard();
    let mut recorder = Recorder::new("contract:v1", Digest::sha256(b"s0")).unwrap();
    recorder
        .record_step(
            Digest::sha256(b"in"),
            Digest::sha256(b"out"),
            Digest::sha256(b"s1"),
            1,
            vec![],
            PURE_CAPABILITY,
            budget,
        )
        .unwrap();
    recorder
        .record_step(
            Digest::sha256(b"in2"),
            Digest::sha256(b"out2"),
            Digest::sha256(b"s2"),
            2,
            vec![0],
            PURE_CAPABILITY,
            budget,
        )
        .unwrap();
    let trace = recorder.finish().unwrap();
    assert_eq!(independent_trace_digest(&trace).unwrap(), trace.digest);
    Replayer::new().replay(&trace, budget).unwrap();
    let (train, held) = partition_held_out(&trace).unwrap();
    assert_eq!(train.coverage.held_out_steps, 1);
    assert_eq!(held.steps.len(), 1);
    let signing = SigningKey::from_bytes("k1", [17_u8; 32]);
    let sig = sign_trace(&trace, &signing).unwrap();
    verify_trace(&trace, &sig, &signing.public_bytes()).unwrap();

    let mut before_g = SemanticGraph::default();
    before_g
        .nodes
        .insert("svc".into(), ("service".into(), Digest::sha256(b"v1")));
    let mut after_g = before_g.clone();
    after_g
        .nodes
        .insert("svc".into(), ("service".into(), Digest::sha256(b"v2")));
    let snapper = GraphSnapshotter {
        ontology_version: anasemble_core::BoundedText::new("onto-1").unwrap(),
        subject: anasemble_core::BoundedText::new("system:demo").unwrap(),
    };
    let cut = TemporalCut::new("2026-08-25T00:00:00Z", "2026-08-25T00:00:01Z");
    let before = snapper.snapshot(&before_g, cut.clone(), budget).unwrap();
    let after = snapper.snapshot(&after_g, cut, budget).unwrap();
    assert_eq!(independent_snapshot_digest(&before).unwrap(), before.digest);
    let again = snapper
        .snapshot(
            &before_g,
            TemporalCut::new("2026-08-25T00:00:00Z", "2026-08-25T00:00:01Z"),
            budget,
        )
        .unwrap();
    assert_eq!(before.digest, again.digest);
    let diff = CanonicalDiffer
        .diff(&before, &after, &DiffPolicy::default())
        .unwrap();
    assert_eq!(diff.operations.len(), 1);

    let contract =
        ContractV1::new("1.0.0", None, CompatibilityClass::Backward, Vec::new()).unwrap();
    let verifier = DigestEqualityVerifier {
        oracle_id: "digest-eq".into(),
        oracle_version: "1".into(),
    };
    let report = verifier
        .verify(
            &contract,
            &DigestCandidate {
                digest: contract.digest,
            },
            &anasemble_verification::EvidenceSet {
                roots: vec![Digest::sha256(b"obs")],
            },
            budget,
        )
        .unwrap();
    assert_eq!(independent_report_digest(&report).unwrap(), report.digest);
    let cert = sign_report(&report, &signing).unwrap();
    verify_certificate(&report, &cert, &signing.public_bytes()).unwrap();
}

#[test]
fn resource_bounds_and_side_effects_refuse() {
    let tiny = Budget::new(1, 1, 1, 8, 10).unwrap();
    let mut graph = SemanticGraph::default();
    graph
        .nodes
        .insert("a".into(), ("s".into(), Digest::sha256(b"a")));
    graph
        .nodes
        .insert("b".into(), ("s".into(), Digest::sha256(b"b")));
    let snapper = GraphSnapshotter {
        ontology_version: anasemble_core::BoundedText::new("onto-1").unwrap(),
        subject: anasemble_core::BoundedText::new("system:demo").unwrap(),
    };
    assert!(
        snapper
            .snapshot(
                &graph,
                TemporalCut::new("2026-08-25T00:00:00Z", "2026-08-25T00:00:01Z"),
                tiny
            )
            .is_err()
    );
    let mut recorder = Recorder::new("c", Digest::sha256(b"s0")).unwrap();
    recorder
        .record_step(
            Digest::sha256(b"in"),
            Digest::sha256(b"net"),
            Digest::sha256(b"s1"),
            1,
            vec![],
            "network.write",
            Budget::standard(),
        )
        .unwrap();
    let trace = recorder.finish().unwrap();
    assert!(Replayer::new().replay(&trace, Budget::standard()).is_err());
}
