use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::canonical::digest;
use crate::model::{Error, RefusalCode};
use crate::protocol::{
    RecoveryMode, RecoveryResult, registered_backup_available, registered_candidate_limit,
    registered_experiment, run, run_with_mode,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignManifest {
    version: String,
    cases: Vec<CampaignCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignCase {
    id: String,
    workspace: String,
    expected: ExpectedDecision,
    expected_candidate_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ExpectedDecision {
    Certified,
    Refused,
    Timeout,
    Disagreement,
    Negative,
}

#[derive(Debug, Serialize)]
pub struct CampaignReport {
    pub version: &'static str,
    pub cases: Vec<CaseReport>,
    pub metrics: CampaignMetrics,
    pub registered_primary_metrics: Vec<String>,
    pub registered_secondary_metrics: Vec<String>,
    pub metric_values: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize)]
pub struct CaseReport {
    pub id: String,
    pub expected: String,
    pub observed: String,
    pub candidate_digest: Option<String>,
    pub baselines: Vec<BaselineReport>,
    pub matched_expectation: bool,
}

#[derive(Debug, Serialize)]
pub struct BaselineReport {
    pub name: String,
    pub observed: String,
}

#[derive(Debug, Default, Serialize)]
pub struct CampaignMetrics {
    pub total_cases: usize,
    pub certified_correct_recoveries: usize,
    pub unsafe_certifications: usize,
    pub refusals: usize,
    pub timeouts: usize,
    pub disagreements: usize,
    pub retained_negative_results: usize,
}

pub fn run_campaign(root: &Path) -> Result<CampaignReport, Error> {
    let manifest_path = root.join("campaign.json");
    let metadata = fs::symlink_metadata(&manifest_path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 131_072 {
        return Err(Error::InvalidRegistry(
            "campaign manifest is not a bounded regular file".into(),
        ));
    }
    let manifest: CampaignManifest = serde_json::from_slice(&fs::read(manifest_path)?)
        .map_err(|error| Error::InvalidRegistry(format!("campaign JSON is invalid: {error}")))?;
    if manifest.version != "campaign-v1" || manifest.cases.is_empty() || manifest.cases.len() > 256
    {
        return Err(Error::InvalidRegistry(
            "campaign version or case count is invalid".into(),
        ));
    }
    let mut metrics = CampaignMetrics::default();
    let mut reports = Vec::with_capacity(manifest.cases.len());
    let mut registration = None;
    let mut search_time_micros = 0_u64;
    let mut candidate_complexity = 0_u64;
    let mut authoring_bytes = 0_u64;
    let mut candidate_work_budget = 0_u64;
    for case in manifest.cases {
        validate_case(&case)?;
        let workspace = root.join(&case.workspace);
        let workspace_metadata = fs::symlink_metadata(&workspace)?;
        if !workspace_metadata.is_dir() || workspace_metadata.file_type().is_symlink() {
            return Err(Error::InvalidRegistry(
                "campaign workspace must be a real directory".into(),
            ));
        }
        authoring_bytes = authoring_bytes
            .checked_add(evidence_bytes(&workspace)?)
            .ok_or_else(|| Error::SearchExhausted("campaign byte counter overflow".into()))?;
        candidate_work_budget = candidate_work_budget
            .checked_add(
                registered_candidate_limit(&workspace)?
                    .checked_mul(3)
                    .ok_or_else(|| {
                        Error::SearchExhausted("campaign work budget overflow".into())
                    })?,
            )
            .ok_or_else(|| Error::SearchExhausted("campaign work budget overflow".into()))?;
        if candidate_work_budget > 4_000_000 {
            return Err(Error::SearchExhausted(
                "campaign exceeds four million registered candidate evaluations".into(),
            ));
        }
        let started = Instant::now();
        let result = run(&workspace);
        search_time_micros = search_time_micros
            .saturating_add(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
        let observed = classify(&result);
        let candidate_digest = match &result {
            RecoveryResult::Certified { candidate, .. } => Some(digest(candidate.as_ref())?),
            RecoveryResult::Refused { .. } => None,
        };
        let matched = matches_expected(case.expected, &result)
            && case
                .expected_candidate_digest
                .as_ref()
                .is_none_or(|expected| candidate_digest.as_ref() == Some(expected));
        metrics.total_cases += 1;
        if matches!(result, RecoveryResult::Certified { .. }) {
            if let RecoveryResult::Certified { candidate, .. } = &result {
                candidate_complexity = candidate_complexity
                    .saturating_add(u64::try_from(candidate.transitions.len()).unwrap_or(u64::MAX));
            }
            if matched {
                metrics.certified_correct_recoveries += 1;
            } else {
                metrics.unsafe_certifications += 1;
            }
        } else {
            metrics.refusals += 1;
        }
        if case.expected == ExpectedDecision::Timeout && matched {
            metrics.timeouts += 1;
        }
        if case.expected == ExpectedDecision::Disagreement && matched {
            metrics.disagreements += 1;
        }
        if case.expected == ExpectedDecision::Negative && matched {
            metrics.retained_negative_results += 1;
        }
        let experiment = registered_experiment(&workspace)?;
        if let Some(previous) = &registration {
            if previous
                != &(
                    experiment.baselines.clone(),
                    experiment.primary_metrics.clone(),
                    experiment.secondary_metrics.clone(),
                )
            {
                return Err(Error::InvalidRegistry(
                    "campaign cases do not share one matched experiment registration".into(),
                ));
            }
        } else {
            registration = Some((
                experiment.baselines.clone(),
                experiment.primary_metrics.clone(),
                experiment.secondary_metrics.clone(),
            ));
        }
        let baselines = execute_baselines(&workspace, experiment.baselines)?;
        reports.push(CaseReport {
            id: case.id,
            expected: expected_name(case.expected).into(),
            observed,
            candidate_digest,
            baselines,
            matched_expectation: matched,
        });
    }
    let (_, primary_metrics, secondary_metrics) = registration.expect("campaign is non-empty");
    let metric_values = metric_values(
        &primary_metrics,
        &secondary_metrics,
        &metrics,
        search_time_micros,
        candidate_complexity,
        authoring_bytes,
    )?;
    Ok(CampaignReport {
        version: "campaign-report-v1",
        cases: reports,
        metrics,
        registered_primary_metrics: primary_metrics,
        registered_secondary_metrics: secondary_metrics,
        metric_values,
    })
}

fn metric_values(
    primary: &[String],
    secondary: &[String],
    metrics: &CampaignMetrics,
    search_time_micros: u64,
    candidate_complexity: u64,
    authoring_bytes: u64,
) -> Result<BTreeMap<String, u64>, Error> {
    let refusal_basis_points = if metrics.total_cases == 0 {
        0
    } else {
        u64::try_from(metrics.refusals)
            .unwrap_or(u64::MAX)
            .saturating_mul(10_000)
            / u64::try_from(metrics.total_cases).unwrap_or(u64::MAX)
    };
    let available = BTreeMap::from([
        (
            "certified-correct-recoveries",
            u64::try_from(metrics.certified_correct_recoveries).unwrap_or(u64::MAX),
        ),
        (
            "unsafe-certifications",
            u64::try_from(metrics.unsafe_certifications).unwrap_or(u64::MAX),
        ),
        ("refusal-rate", refusal_basis_points),
        ("search-time", search_time_micros),
        ("candidate-complexity", candidate_complexity),
        ("authoring-cost", authoring_bytes),
    ]);
    primary
        .iter()
        .chain(secondary)
        .map(|name| {
            available
                .get(name.as_str())
                .copied()
                .map(|value| (name.clone(), value))
                .ok_or_else(|| {
                    Error::InvalidRegistry(format!("unsupported registered metric: {name}"))
                })
        })
        .collect()
}

fn evidence_bytes(workspace: &Path) -> Result<u64, Error> {
    let registry = fs::symlink_metadata(workspace.join("registry.json"))?;
    if !registry.is_file() || registry.file_type().is_symlink() || registry.len() > 131_072 {
        return Err(Error::InvalidEvidence(
            "campaign registry is not a bounded regular file".into(),
        ));
    }
    let mut total = registry.len();
    let mut count = 0_usize;
    for entry in fs::read_dir(workspace.join("fragments"))? {
        count += 1;
        if count > 10_000 {
            return Err(Error::SearchExhausted(
                "campaign fragment bound exceeded".into(),
            ));
        }
        let metadata = fs::symlink_metadata(entry?.path())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 1_048_576 {
            return Err(Error::InvalidEvidence(
                "campaign evidence is not a bounded regular file".into(),
            ));
        }
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| Error::SearchExhausted("campaign byte counter overflow".into()))?;
    }
    Ok(total)
}

fn execute_baselines(
    workspace: &Path,
    baselines: Vec<String>,
) -> Result<Vec<BaselineReport>, Error> {
    baselines
        .into_iter()
        .map(|name| {
            let observed = match name.as_str() {
                "backup-replica" => {
                    if registered_backup_available(workspace)? {
                        "available".into()
                    } else {
                        "unavailable_after_registered_total_loss".into()
                    }
                }
                "trace-only" => classify(&run_with_mode(workspace, RecoveryMode::TraceOnly)),
                "centralized-contract" => {
                    classify(&run_with_mode(workspace, RecoveryMode::CentralizedContract))
                }
                _ => {
                    return Err(Error::InvalidRegistry(format!(
                        "unsupported registered baseline: {name}"
                    )));
                }
            };
            Ok(BaselineReport { name, observed })
        })
        .collect()
}

fn classify(result: &RecoveryResult) -> String {
    match result {
        RecoveryResult::Certified { .. } => "certified".into(),
        RecoveryResult::Refused { code, .. } => format!("refused:{code:?}"),
    }
}

fn matches_expected(expected: ExpectedDecision, result: &RecoveryResult) -> bool {
    matches!(
        (expected, result),
        (
            ExpectedDecision::Certified,
            RecoveryResult::Certified { .. }
        ) | (
            ExpectedDecision::Refused | ExpectedDecision::Negative,
            RecoveryResult::Refused { .. },
        ) | (
            ExpectedDecision::Timeout,
            RecoveryResult::Refused {
                code: RefusalCode::SearchExhausted,
                ..
            },
        ) | (
            ExpectedDecision::Disagreement,
            RecoveryResult::Refused {
                code: RefusalCode::CheckerRejected,
                ..
            },
        )
    )
}

fn validate_case(case: &CampaignCase) -> Result<(), Error> {
    let path = Path::new(&case.workspace);
    let digest_required = case.expected == ExpectedDecision::Certified;
    if case.id.is_empty()
        || case.id.len() > 128
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
        || case
            .expected_candidate_digest
            .as_ref()
            .is_some_and(|digest| digest.len() != 64 || hex::decode(digest).is_err())
        || digest_required != case.expected_candidate_digest.is_some()
    {
        return Err(Error::InvalidRegistry("campaign case is invalid".into()));
    }
    Ok(())
}

fn expected_name(expected: ExpectedDecision) -> &'static str {
    match expected {
        ExpectedDecision::Certified => "certified",
        ExpectedDecision::Refused => "refused",
        ExpectedDecision::Timeout => "timeout",
        ExpectedDecision::Disagreement => "disagreement",
        ExpectedDecision::Negative => "negative",
    }
}
