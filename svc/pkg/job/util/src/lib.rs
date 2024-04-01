use std::time::Duration;

pub mod key;

/// Determines if a Nomad job is dispatched from our run.
///
/// We use this when monitoring Nomad in order to determine which events to
/// pay attention to.
pub fn is_nomad_job_run(job_id: &str) -> bool {
	job_id.starts_with("job-") && job_id.contains("/dispatch-")
}

// Timeout from when `stop_job` is called and the kill signal is sent
pub const JOB_STOP_TIMEOUT: Duration = Duration::from_secs(30);

pub const TASK_CLEANUP_CPU: i32 = 50;

// Query Prometheus with:
//
// ```
// max(nomad_client_allocs_memory_max_usage{ns="prod",exported_job=~"job-.*",task="run-cleanup"}) / 1000 / 1000
// ```
//
// 13.5 MB baseline, 29 MB highest peak
pub const TASK_CLEANUP_MEMORY: i32 = 32;

pub const RUN_MAIN_TASK_NAME: &str = "main";
pub const RUN_CLEANUP_TASK_NAME: &str = "run-cleanup";
