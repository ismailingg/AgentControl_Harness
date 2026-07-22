use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub type TraceResult<T> = Result<T, TraceError>;

#[derive(Debug)]
pub enum TraceError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TraceError::Io(error) => write!(f, "I/O error: {error}"),
            TraceError::Json(error) => write!(f, "JSON error: {error}"),
        }
    }
}

impl std::error::Error for TraceError {}

impl From<io::Error> for TraceError {
    fn from(error: io::Error) -> Self {
        TraceError::Io(error)
    }
}

impl From<serde_json::Error> for TraceError {
    fn from(error: serde_json::Error) -> Self {
        TraceError::Json(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEvent {
    pub run_id: String,
    pub seq: u64,
    pub timestamp: String,
    #[serde(flatten)]
    pub kind: TraceEventKind,
}

impl TraceEvent {
    pub fn new(run_id: impl Into<String>, seq: u64, kind: TraceEventKind) -> Self {
        Self {
            run_id: run_id.into(),
            seq,
            timestamp: current_timestamp(),
            kind,
        }
    }

    pub fn event_type(&self) -> TraceEventType {
        self.kind.event_type()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum TraceEventKind {
    RunStarted(RunStartedPayload),
    PolicyDecision(PolicyDecisionPayload),
    TerminalCommand(TerminalCommandPayload),
    ConfirmationResponse(ConfirmationResponsePayload),
    RunFinished(RunFinishedPayload),
    Error(ErrorPayload),
}

impl TraceEventKind {
    pub fn event_type(&self) -> TraceEventType {
        match self {
            TraceEventKind::RunStarted(_) => TraceEventType::RunStarted,
            TraceEventKind::PolicyDecision(_) => TraceEventType::PolicyDecision,
            TraceEventKind::TerminalCommand(_) => TraceEventType::TerminalCommand,
            TraceEventKind::ConfirmationResponse(_) => TraceEventType::ConfirmationResponse,
            TraceEventKind::RunFinished(_) => TraceEventType::RunFinished,
            TraceEventKind::Error(_) => TraceEventType::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventType {
    RunStarted,
    PolicyDecision,
    TerminalCommand,
    ConfirmationResponse,
    RunFinished,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStartedPayload {
    pub agentharness_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecisionPayload {
    pub command: String,
    pub action: String,
    pub risk: String,
    pub rule: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCommandPayload {
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub stdout_excerpt: Option<String>,
    pub stderr_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmationResponsePayload {
    pub command: String,
    pub approved: bool,
    pub responder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunFinishedPayload {
    pub status: RunStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub message: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Success,
    Failed,
    Blocked,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunMetadata {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: RunStatus,
    pub agentharness_version: String,
}

pub struct TraceWriter {
    runs_root: PathBuf,
    run_dir: PathBuf,
    metadata: RunMetadata,
    next_seq: u64,
}

impl TraceWriter {
    pub fn create(
        runs_root: impl AsRef<Path>,
        agentharness_version: impl Into<String>,
    ) -> TraceResult<Self> {
        let runs_root = runs_root.as_ref().to_path_buf();
        let agentharness_version = agentharness_version.into();
        let run_id = new_run_id();
        let run_dir = runs_root.join(&run_id);
        let metadata = RunMetadata {
            run_id,
            started_at: current_timestamp(),
            finished_at: None,
            status: RunStatus::Running,
            agentharness_version,
        };

        fs::create_dir_all(&run_dir)?;
        File::create(run_dir.join("events.jsonl"))?;
        write_metadata(&run_dir, &metadata)?;
        fs::create_dir_all(&runs_root)?;
        fs::write(runs_root.join("latest.txt"), metadata.run_id.as_bytes())?;

        Ok(Self {
            runs_root,
            run_dir,
            metadata,
            next_seq: 1,
        })
    }

    pub fn append(&mut self, kind: TraceEventKind) -> TraceResult<u64> {
        let seq = self.next_seq;
        let event = TraceEvent::new(self.metadata.run_id.clone(), seq, kind);
        let mut events = OpenOptions::new().append(true).open(self.events_path())?;

        serde_json::to_writer(&mut events, &event)?;
        events.write_all(b"\n")?;
        events.sync_data()?;
        self.next_seq += 1;

        Ok(seq)
    }

    pub fn finish(&mut self, status: RunStatus) -> TraceResult<()> {
        self.metadata.status = status;
        self.metadata.finished_at = Some(current_timestamp());
        write_metadata(&self.run_dir, &self.metadata)
    }

    pub fn run_id(&self) -> &str {
        &self.metadata.run_id
    }

    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    pub fn runs_root(&self) -> &Path {
        &self.runs_root
    }

    pub fn metadata(&self) -> &RunMetadata {
        &self.metadata
    }

    pub fn events_path(&self) -> PathBuf {
        self.run_dir.join("events.jsonl")
    }
}

pub fn read_events(path: impl AsRef<Path>) -> TraceResult<Vec<TraceEvent>> {
    let contents = fs::read_to_string(path)?;
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(TraceError::from))
        .collect()
}

fn write_metadata(run_dir: &Path, metadata: &RunMetadata) -> TraceResult<()> {
    let mut metadata_file = File::create(run_dir.join("metadata.json"))?;
    serde_json::to_writer_pretty(&mut metadata_file, metadata)?;
    metadata_file.write_all(b"\n")?;
    metadata_file.sync_data()?;
    Ok(())
}

fn new_run_id() -> String {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let uuid = Uuid::new_v4().simple().to_string();

    format!("run_{timestamp}_{}", &uuid[..4])
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn serializes_policy_decision_with_common_envelope() {
        let event = TraceEvent {
            run_id: "run_20260721_120000_a1b2".to_owned(),
            seq: 2,
            timestamp: "2026-07-21T12:00:00Z".to_owned(),
            kind: TraceEventKind::PolicyDecision(PolicyDecisionPayload {
                command: "sudo RM -Rf /".to_owned(),
                action: "block".to_owned(),
                risk: "critical".to_owned(),
                rule: "destructive-root-delete".to_owned(),
                reason: "command attempts a destructive delete against a root/system path"
                    .to_owned(),
            }),
        };

        let json = serde_json::to_string(&event).expect("event should serialize");

        assert!(json.contains(r#""type":"policy_decision""#));
        assert!(json.contains(r#""run_id":"run_20260721_120000_a1b2""#));
        assert!(json.contains(r#""seq":2"#));
        assert!(json.contains(r#""payload":{"command":"sudo RM -Rf /""#));

        let decoded: TraceEvent = serde_json::from_str(&json).expect("event should deserialize");
        assert_eq!(decoded, event);
    }

    #[test]
    fn deserializes_confirmation_response_by_event_type_not_payload_shape() {
        let json = r#"{"run_id":"run_20260721_120000_a1b2","seq":3,"timestamp":"2026-07-21T12:00:01Z","type":"confirmation_response","payload":{"command":"rm -rf target","approved":true,"responder":null}}"#;

        let event: TraceEvent =
            serde_json::from_str(json).expect("confirmation response should deserialize");

        assert_eq!(event.event_type(), TraceEventType::ConfirmationResponse);
        assert!(matches!(
            event.kind,
            TraceEventKind::ConfirmationResponse(ConfirmationResponsePayload {
                command,
                approved: true,
                responder: None,
            }) if command == "rm -rf target"
        ));
    }

    #[test]
    fn creates_run_directory_metadata_events_and_latest_pointer() {
        let runs_root = test_runs_root("create_run");
        let writer = TraceWriter::create(&runs_root, "0.1.0").expect("run should be created");

        assert!(writer.run_id().starts_with("run_"));
        assert!(writer.run_dir().exists());
        assert!(writer.run_dir().join("metadata.json").exists());
        assert!(writer.run_dir().join("events.jsonl").exists());

        let latest = fs::read_to_string(runs_root.join("latest.txt")).expect("latest should exist");
        assert_eq!(latest, writer.run_id());

        let metadata: RunMetadata = serde_json::from_str(
            &fs::read_to_string(writer.run_dir().join("metadata.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata.run_id, writer.run_id());
        assert_eq!(metadata.status, RunStatus::Running);
        assert_eq!(metadata.agentharness_version, "0.1.0");

        fs::remove_dir_all(runs_root).ok();
    }

    #[test]
    fn appends_events_synchronously_with_incrementing_sequence() {
        let runs_root = test_runs_root("append_events");
        let mut writer = TraceWriter::create(&runs_root, "0.1.0").expect("run should be created");

        let first_seq = writer
            .append(TraceEventKind::RunStarted(RunStartedPayload {
                agentharness_version: "0.1.0".to_owned(),
            }))
            .expect("first event should append");
        let second_seq = writer
            .append(TraceEventKind::PolicyDecision(PolicyDecisionPayload {
                command: "rm -rf /".to_owned(),
                action: "block".to_owned(),
                risk: "critical".to_owned(),
                rule: "destructive-root-delete".to_owned(),
                reason: "command attempts a destructive delete against a root/system path"
                    .to_owned(),
            }))
            .expect("second event should append");

        assert_eq!(first_seq, 1);
        assert_eq!(second_seq, 2);

        let events = read_events(writer.events_path()).expect("events should read");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].seq, 1);
        assert_eq!(events[1].seq, 2);
        assert_eq!(events[1].event_type(), TraceEventType::PolicyDecision);

        fs::remove_dir_all(runs_root).ok();
    }

    #[test]
    fn finish_updates_metadata_status_and_finished_at() {
        let runs_root = test_runs_root("finish_run");
        let mut writer = TraceWriter::create(&runs_root, "0.1.0").expect("run should be created");

        writer
            .finish(RunStatus::Blocked)
            .expect("run should finish");

        let metadata: RunMetadata = serde_json::from_str(
            &fs::read_to_string(writer.run_dir().join("metadata.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata.status, RunStatus::Blocked);
        assert!(metadata.finished_at.is_some());

        fs::remove_dir_all(runs_root).ok();
    }

    fn test_runs_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("agentharness_trace_{name}_{nanos}"))
    }
}
