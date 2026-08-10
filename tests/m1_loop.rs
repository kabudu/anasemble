mod common;

use std::fs;
use std::process::Command;

use anasemble::checker::certify;
use anasemble::checker_wire::encode_candidate;
use anasemble::fragments::FragmentKind;
use anasemble::ledger::persist;
use anasemble::model::{Candidate, Error, FragmentContent, Grammar, TraceRole, Transition};
use anasemble::protocol::{RecoveryResult, run};
use anasemble::sandbox::{compile, execute_untrusted_step, rejects_capabilities, verify};
use anasemble::synthesizer::reconstruct;
use common::{CONTRACT_KEY, STATE_KEY, build_workspace, write_json};
use serde_json::{Value, json};
use tempfile::tempdir;
use wasm_encoder::{
    BlockType, CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection,
    ImportSection, Instruction, MemorySection, MemoryType, Module, TypeSection, ValType,
};

#[test]
fn fsm_v1_enumerates_a_unique_candidate_and_refuses_ambiguity() {
    let grammar = Grammar {
        version: "fsm-v1".into(),
        inputs: vec!["request".into()],
        outputs: vec!["deny".into(), "allow".into()],
        states: vec!["ready".into()],
        initial_state: "ready".into(),
        max_candidates: 2,
    };
    let contract = FragmentContent::Transition {
        state: "ready".into(),
        input: "request".into(),
        next_state: "ready".into(),
        output: "allow".into(),
    };
    let (candidate, examined) = reconstruct("gate", "1", &grammar, &[contract]).unwrap();
    assert_eq!(candidate.transitions[0].output, "allow");
    assert_eq!(examined, 2);
    let error = reconstruct("gate", "1", &grammar, &[]).unwrap_err();
    assert!(matches!(error, Error::InsufficientEvidence(_)));
}

#[test]
fn training_trace_constrains_synthesis_but_held_out_trace_does_not() {
    let grammar = Grammar {
        version: "fsm-v1".into(),
        inputs: vec!["request".into()],
        outputs: vec!["deny".into(), "allow".into()],
        states: vec!["ready".into()],
        initial_state: "ready".into(),
        max_candidates: 2,
    };
    let training = FragmentContent::Trace {
        role: TraceRole::Training,
        initial_state: "ready".into(),
        inputs: vec!["request".into()],
        outputs: vec!["allow".into()],
    };
    let held_out = FragmentContent::Trace {
        role: TraceRole::HeldOut,
        initial_state: "ready".into(),
        inputs: vec!["request".into()],
        outputs: vec!["allow".into()],
    };
    assert!(reconstruct("gate", "1", &grammar, &[training]).is_ok());
    assert!(matches!(
        reconstruct("gate", "1", &grammar, &[held_out]),
        Err(Error::InsufficientEvidence(_))
    ));
}

#[test]
fn generated_wasm_has_no_imports_and_matches_every_transition() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    assert!(!workspace.artifact.exists());
    assert_eq!(workspace.artifact_digest.len(), 64);
    let RecoveryResult::Certified {
        candidate,
        candidate_wasm_hex,
        ..
    } = run(&workspace.recovery)
    else {
        panic!("expected certification");
    };
    let wasm = hex::decode(candidate_wasm_hex).unwrap();
    let evidence = verify(&candidate, &wasm).unwrap();
    assert_eq!(evidence.imports, 0);
    assert_eq!(evidence.verified_executions, candidate.transitions.len());
    assert_eq!(compile(&candidate).unwrap(), wasm);
}

#[test]
fn sandbox_rejects_any_imported_capability() {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function([], [ValType::I32]);
    module.section(&types);
    let mut imports = ImportSection::new();
    imports.import("ambient", "clock", EntityType::Function(0));
    module.section(&imports);
    let error = rejects_capabilities(&module.finish()).unwrap_err();
    assert!(matches!(error, Error::CheckerRejected(_)));
}

#[test]
fn sandbox_stops_an_infinite_candidate_at_the_fuel_bound() {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I64]);
    module.section(&types);
    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);
    let mut exports = ExportSection::new();
    exports.export("step", ExportKind::Func, 0);
    module.section(&exports);
    let mut function = Function::new([]);
    function.instruction(&Instruction::Loop(BlockType::Empty));
    function.instruction(&Instruction::Br(0));
    function.instruction(&Instruction::End);
    function.instruction(&Instruction::I64Const(0));
    function.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&function);
    module.section(&code);
    let error = execute_untrusted_step(&module.finish(), 0, 0, 100).unwrap_err();
    assert!(matches!(error, Error::SearchExhausted(_)));
}

#[test]
fn sandbox_rejects_memory_above_the_compiled_limit() {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I64]);
    module.section(&types);
    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 2,
        maximum: Some(2),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memories);
    let mut exports = ExportSection::new();
    exports.export("step", ExportKind::Func, 0);
    module.section(&exports);
    let mut function = Function::new([]);
    function.instruction(&Instruction::I64Const(0));
    function.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&function);
    module.section(&code);
    assert!(execute_untrusted_step(&module.finish(), 0, 0, 100).is_err());
}

#[test]
fn negative_and_metamorphic_obligations_are_executable() {
    let grammar = Grammar {
        version: "fsm-v1".into(),
        inputs: vec!["set".into()],
        outputs: vec!["ok".into(), "bad".into()],
        states: vec!["ready".into()],
        initial_state: "ready".into(),
        max_candidates: 2,
    };
    let contents = vec![
        FragmentContent::Transition {
            state: "ready".into(),
            input: "set".into(),
            next_state: "ready".into(),
            output: "ok".into(),
        },
        FragmentContent::NegativeCase {
            initial_state: "ready".into(),
            inputs: vec!["set".into()],
            forbidden_outputs: vec!["bad".into()],
        },
        FragmentContent::MetamorphicProperty {
            initial_state: "ready".into(),
            input: "set".into(),
            repetitions: 3,
        },
    ];
    let (candidate, _) = reconstruct("setter", "1", &grammar, &contents).unwrap();
    let mut checker_contents = contents;
    checker_contents.push(FragmentContent::StatePolicy {
        states: vec!["ready".into()],
        initial_state: "ready".into(),
    });
    checker_contents.push(FragmentContent::Trace {
        role: TraceRole::HeldOut,
        initial_state: "ready".into(),
        inputs: vec!["set".into()],
        outputs: vec!["ok".into()],
    });
    let coverage = certify(
        &encode_candidate(&candidate).unwrap(),
        "setter",
        "1",
        &checker_contents,
    )
    .unwrap();
    assert_eq!(coverage.passed_negative_cases, 1);
    assert_eq!(coverage.passed_metamorphic_properties, 1);
}

#[test]
fn checker_wire_rejects_truncation_and_trailing_data() {
    let candidate = Candidate {
        component: "inverter".into(),
        interface_version: "1".into(),
        grammar: stateless_grammar(),
        transitions: stateless_transitions(false),
    };
    let mut wire = encode_candidate(&candidate).unwrap();
    assert!(certify(&wire[..wire.len() - 1], "inverter", "1", &[]).is_err());
    wire.push(0);
    assert!(certify(&wire, "inverter", "1", &[]).is_err());
}

#[test]
fn ledger_is_immutable_and_replay_stable() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    let result = run(&workspace.recovery);
    let ledger = directory.path().join("ledger");
    let first = persist(&workspace.recovery, &ledger, &result).unwrap();
    let second = persist(&workspace.recovery, &ledger, &result).unwrap();
    assert!(!first.replay);
    assert!(second.replay);
    assert_eq!(first.entry_id, second.entry_id);
    assert!(first.path.join("manifest.json").is_file());
    assert!(first.path.join("inputs/registry.json").is_file());
    assert_eq!(
        fs::read(first.path.join("outcome.json")).unwrap(),
        anasemble::canonical::encode(&result).unwrap()
    );
    fs::write(first.path.join("outcome.json"), b"corrupt").unwrap();
    assert!(persist(&workspace.recovery, &ledger, &result).is_err());
}

#[test]
fn public_recover_cli_persists_a_ledger_entry() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    let ledger = directory.path().join("ledger");
    let output = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .arg("recover")
        .arg(&workspace.recovery)
        .arg("--ledger")
        .arg(&ledger)
        .env_clear()
        .output()
        .unwrap();
    assert!(output.status.success());
    let entries: Vec<_> = fs::read_dir(&ledger).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert!(
        entries[0]
            .as_ref()
            .unwrap()
            .path()
            .join("manifest.json")
            .is_file()
    );
}

#[test]
fn public_corpus_cli_recovers_two_distinct_stateless_components() {
    let directory = tempdir().unwrap();
    let root = directory.path().join("corpus");
    fs::create_dir(&root).unwrap();
    build_stateless(&root, "inverter", false);
    build_stateless(&root, "identity", true);
    write_json(
        &root.join("corpus.json"),
        &json!({"version": "corpus-v1", "workspaces": ["identity", "inverter"]}),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .arg("recover-corpus")
        .arg(&root)
        .env_clear()
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["results"].as_array().unwrap().len(), 2);
    assert!(
        value["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["result"]["decision"] == "CERTIFIED")
    );
}

fn stateless_grammar() -> Grammar {
    Grammar {
        version: "fsm-v1".into(),
        inputs: vec!["false".into(), "true".into()],
        outputs: vec!["false".into(), "true".into()],
        states: vec!["ready".into()],
        initial_state: "ready".into(),
        max_candidates: 4,
    }
}

fn stateless_transitions(identity: bool) -> Vec<Transition> {
    ["false", "true"]
        .into_iter()
        .map(|input| Transition {
            state: "ready".into(),
            input: input.into(),
            next_state: "ready".into(),
            output: if identity {
                input.into()
            } else if input == "false" {
                "true".into()
            } else {
                "false".into()
            },
        })
        .collect()
}

fn build_stateless(root: &std::path::Path, component: &str, identity: bool) {
    let workspace = root.join(component);
    let fragments = workspace.join("fragments");
    let origin = root.join(format!(".{component}-lost"));
    fs::create_dir_all(&fragments).unwrap();
    fs::create_dir(&origin).unwrap();
    let artifact = origin.join("component.json");
    write_json(&artifact, &stateless_transitions(identity));
    let artifact_digest = anasemble::canonical::bytes_digest(&fs::read(&artifact).unwrap());
    for (sequence, transition) in stateless_transitions(identity).into_iter().enumerate() {
        write_component_envelope(
            &fragments.join(format!("contract-{sequence}.json")),
            component,
            FragmentKind::Contract,
            sequence as u64,
            FragmentContent::Transition {
                state: transition.state,
                input: transition.input,
                next_state: transition.next_state,
                output: transition.output,
            },
            &CONTRACT_KEY,
            "contract-authority",
            "domain-a",
        );
    }
    write_component_envelope(
        &fragments.join("state.json"),
        component,
        FragmentKind::StateSchema,
        0,
        FragmentContent::StatePolicy {
            states: vec!["ready".into()],
            initial_state: "ready".into(),
        },
        &STATE_KEY,
        "state-authority",
        "domain-b",
    );
    let outputs: Vec<_> = stateless_transitions(identity)
        .into_iter()
        .map(|item| item.output)
        .collect();
    write_component_envelope(
        &fragments.join("trace.json"),
        component,
        FragmentKind::Trace,
        1,
        FragmentContent::Trace {
            role: TraceRole::HeldOut,
            initial_state: "ready".into(),
            inputs: vec!["false".into(), "true".into()],
            outputs,
        },
        &STATE_KEY,
        "state-authority",
        "domain-b",
    );
    write_json(
        &workspace.join("registry.json"),
        &json!({
            "component": component, "interface_version": "1", "grammar": stateless_grammar(), "required_domains": 2,
            "trusted_issuers": {"contract-authority": {"hmac_sha256_key": hex::encode(CONTRACT_KEY), "failure_domain": "domain-a"}, "state-authority": {"hmac_sha256_key": hex::encode(STATE_KEY), "failure_domain": "domain-b"}},
            "loss_oracle": {"forbidden_paths": [artifact, origin], "forbidden_sha256": [artifact_digest]},
            "resource_limits": {"max_fragments": 16, "max_fragment_bytes": 16384, "max_workspace_files": 32, "max_workspace_bytes": 262144},
            "experiment": {"seed": 20260810, "baselines": ["trace-only"], "primary_metrics": ["certified-correct-recoveries"], "secondary_metrics": ["search-time"]}
        }),
    );
    fs::remove_dir_all(origin).unwrap();
}

#[allow(clippy::too_many_arguments)]
fn write_component_envelope(
    path: &std::path::Path,
    component: &str,
    kind: FragmentKind,
    sequence: u64,
    content: FragmentContent,
    key: &[u8; 32],
    issuer: &str,
    domain: &str,
) {
    let envelope = anasemble::fragments::sign(
        anasemble::fragments::Envelope {
            kind,
            component: component.into(),
            interface_version: "1".into(),
            issuer: issuer.into(),
            failure_domain: domain.into(),
            issued_at: "2026-08-10T00:00:00+00:00".into(),
            sequence,
            content_digest: String::new(),
            dependencies: Vec::new(),
            content,
            signature: String::new(),
        },
        key,
    )
    .unwrap();
    write_json(path, &envelope);
}
