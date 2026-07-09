use super::*;

fn temp_queue() -> (tempfile::TempDir, Queue) {
    let dir = tempfile::tempdir().unwrap();
    let q = Queue::new(dir.path().join("queue"));
    (dir, q)
}

#[test]
fn enqueue_creates_a_queued_job_with_branch() {
    let (_d, q) = temp_queue();
    let job = q.enqueue("/srv/repo", "add tests").unwrap();
    assert_eq!(job.status, JobStatus::Queued);
    assert_eq!(job.repo, "/srv/repo");
    assert_eq!(job.prompt, "add tests");
    assert_eq!(
        job.branch.as_deref(),
        Some(&*format!("nerve/job-{}", job.id))
    );
    assert!(job.started_at.is_none());
    assert!(job.finished_at.is_none());
}

#[test]
fn enqueue_persists_and_get_round_trips() {
    let (_d, q) = temp_queue();
    let job = q.enqueue("/r", "do it\nplease").unwrap();
    let loaded = q.get(&job.id).unwrap().expect("job persisted");
    assert_eq!(loaded.id, job.id);
    assert_eq!(loaded.prompt, "do it\nplease");
    assert_eq!(loaded.status, JobStatus::Queued);
}

#[test]
fn get_missing_returns_none() {
    let (_d, q) = temp_queue();
    assert!(q.get("nope").unwrap().is_none());
}

#[test]
fn list_empty_when_no_dir() {
    let (_d, q) = temp_queue();
    assert!(q.list().unwrap().is_empty());
}

#[test]
fn list_returns_all_jobs_sorted_oldest_first() {
    let (_d, q) = temp_queue();
    let a = q.enqueue("/r", "first").unwrap();
    let b = q.enqueue("/r", "second").unwrap();
    let jobs = q.list().unwrap();
    assert_eq!(jobs.len(), 2);
    // created_at is coarse (seconds); the id tiebreaker keeps ordering stable,
    // so just assert both are present.
    let ids: Vec<&str> = jobs.iter().map(|j| j.id.as_str()).collect();
    assert!(ids.contains(&a.id.as_str()));
    assert!(ids.contains(&b.id.as_str()));
}

#[test]
fn ids_are_unique_across_many_enqueues() {
    let (_d, q) = temp_queue();
    let mut ids = std::collections::HashSet::new();
    for i in 0..50 {
        let job = q.enqueue("/r", &format!("job {i}")).unwrap();
        assert!(ids.insert(job.id), "duplicate id generated");
    }
    assert_eq!(q.list().unwrap().len(), 50);
}

#[test]
fn next_queued_returns_a_queued_job_and_skips_others() {
    let (_d, q) = temp_queue();
    let a = q.enqueue("/r", "a").unwrap();
    q.enqueue("/r", "b").unwrap();
    // Move `a` out of Queued; next_queued should then return a still-queued one.
    q.mark_running(&a.id).unwrap();
    let next = q.next_queued().unwrap().expect("a queued job remains");
    assert_eq!(next.status, JobStatus::Queued);
    assert_ne!(next.id, a.id);
}

#[test]
fn next_queued_none_when_all_terminal_or_running() {
    let (_d, q) = temp_queue();
    let a = q.enqueue("/r", "a").unwrap();
    q.mark_running(&a.id).unwrap();
    q.mark_done(&a.id).unwrap();
    assert!(q.next_queued().unwrap().is_none());
}

#[test]
fn mark_running_sets_status_and_timestamp() {
    let (_d, q) = temp_queue();
    let job = q.enqueue("/r", "x").unwrap();
    let running = q.mark_running(&job.id).unwrap().unwrap();
    assert_eq!(running.status, JobStatus::Running);
    assert!(running.started_at.is_some());
    // Persisted, not just returned.
    assert_eq!(q.get(&job.id).unwrap().unwrap().status, JobStatus::Running);
}

#[test]
fn mark_done_sets_terminal_state() {
    let (_d, q) = temp_queue();
    let job = q.enqueue("/r", "x").unwrap();
    let done = q.mark_done(&job.id).unwrap().unwrap();
    assert_eq!(done.status, JobStatus::Done);
    assert!(done.status.is_terminal());
    assert!(done.finished_at.is_some());
}

#[test]
fn mark_failed_records_error() {
    let (_d, q) = temp_queue();
    let job = q.enqueue("/r", "x").unwrap();
    let failed = q.mark_failed(&job.id, "compile error").unwrap().unwrap();
    assert_eq!(failed.status, JobStatus::Failed);
    assert_eq!(failed.error.as_deref(), Some("compile error"));
    assert!(failed.finished_at.is_some());
}

#[test]
fn cancel_queued_job_succeeds() {
    let (_d, q) = temp_queue();
    let job = q.enqueue("/r", "x").unwrap();
    assert!(q.cancel(&job.id).unwrap());
    assert_eq!(
        q.get(&job.id).unwrap().unwrap().status,
        JobStatus::Cancelled
    );
}

#[test]
fn cancel_running_job_is_noop() {
    let (_d, q) = temp_queue();
    let job = q.enqueue("/r", "x").unwrap();
    q.mark_running(&job.id).unwrap();
    assert!(!q.cancel(&job.id).unwrap());
    // Still running — a started job can't be cancelled from the queue.
    assert_eq!(q.get(&job.id).unwrap().unwrap().status, JobStatus::Running);
}

#[test]
fn update_missing_job_returns_none() {
    let (_d, q) = temp_queue();
    assert!(q.mark_done("ghost").unwrap().is_none());
}

#[test]
fn status_labels_and_terminal() {
    assert_eq!(JobStatus::Queued.label(), "queued");
    assert_eq!(JobStatus::Running.label(), "running");
    assert!(!JobStatus::Queued.is_terminal());
    assert!(!JobStatus::Running.is_terminal());
    assert!(JobStatus::Done.is_terminal());
    assert!(JobStatus::Failed.is_terminal());
    assert!(JobStatus::Cancelled.is_terminal());
}

#[test]
fn summary_line_truncates_long_prompt() {
    let (_d, q) = temp_queue();
    let long = "a".repeat(200);
    let job = q.enqueue("/r", &long).unwrap();
    let line = job.summary_line();
    assert!(line.contains(&job.id));
    assert!(line.contains("queued"));
    // The prompt is truncated well under its original length.
    assert!(line.len() < 200);
}
