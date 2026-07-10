use super::*;

fn job(id: &str, status: JobStatus) -> Job {
    Job {
        id: id.into(),
        repo: "/srv/repo".into(),
        prompt: "do a thing".into(),
        status,
        branch: Some(format!("nerve/job-{id}")),
        has_context: false,
        created_at: 0,
        started_at: None,
        finished_at: None,
        error: None,
    }
}

#[test]
fn summarize_counts_by_status() {
    let jobs = vec![
        job("a", JobStatus::Queued),
        job("b", JobStatus::Queued),
        job("c", JobStatus::Running),
        job("d", JobStatus::Done),
        job("e", JobStatus::Failed),
        job("f", JobStatus::Cancelled),
    ];
    let s = summarize(&jobs);
    assert_eq!(s.total, 6);
    assert_eq!(s.queued, 2);
    assert_eq!(s.running, 1);
    assert_eq!(s.done, 1);
    assert_eq!(s.failed, 1);
}

#[test]
fn badge_shows_running_and_queued() {
    let s = RemoteStatus {
        running: 2,
        queued: 5,
        total: 7,
        ..Default::default()
    };
    assert_eq!(s.badge(), "2 running \u{00b7} 5 queued");
}

#[test]
fn badge_running_only() {
    let s = RemoteStatus {
        running: 1,
        total: 1,
        ..Default::default()
    };
    assert_eq!(s.badge(), "1 running");
}

#[test]
fn badge_idle_when_nothing_active() {
    let s = RemoteStatus {
        done: 3,
        total: 3,
        ..Default::default()
    };
    assert_eq!(s.badge(), "idle");
    assert_eq!(RemoteStatus::default().badge(), "idle");
}

#[test]
fn parse_jobs_reads_json_array() {
    let json = r#"[{"id":"a1b2c3d4","repo":"/srv/x","prompt":"add tests","status":"queued","branch":"nerve/job-a1b2c3d4","has_context":false,"created_at":100,"started_at":null,"finished_at":null,"error":null}]"#;
    let jobs = parse_jobs(json).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "a1b2c3d4");
    assert_eq!(jobs[0].status, JobStatus::Queued);
}

#[test]
fn parse_jobs_tolerates_leading_noise() {
    let out = "Warning: something\n[{\"id\":\"x\",\"repo\":\"/r\",\"prompt\":\"p\",\"status\":\"running\",\"branch\":null,\"has_context\":false,\"created_at\":1,\"started_at\":2,\"finished_at\":null,\"error\":null}]\n";
    let jobs = parse_jobs(out).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, JobStatus::Running);
}

#[test]
fn parse_jobs_empty_array() {
    assert!(parse_jobs("[]").unwrap().is_empty());
}

#[test]
fn parse_jobs_surfaces_server_error() {
    let err = parse_jobs("ERR could not list jobs: boom").unwrap_err();
    assert!(err.to_string().contains("server error"));
}

#[test]
fn parse_jobs_no_array_errors() {
    assert!(parse_jobs("totally not json").is_err());
}

#[test]
fn rsync_args_exclude_build_artifacts_and_mirror() {
    let args = rsync_args("/home/me/proj", "nerve-server", "/root/nerve-repos/proj");
    assert_eq!(args[0], "-az");
    assert!(args.iter().any(|a| a == "--delete"));
    assert!(args.iter().any(|a| a == "--exclude=node_modules"));
    assert!(args.iter().any(|a| a == "--exclude=target"));
    assert!(args.iter().any(|a| a == "--exclude=.next"));
    // .git and .nerve are NOT excluded — history + project memory travel along.
    assert!(!args.iter().any(|a| a == "--exclude=.git"));
    assert!(!args.iter().any(|a| a == "--exclude=.nerve"));
    // Source has a trailing slash (copy contents); dest is host:remote/.
    assert_eq!(args[args.len() - 2], "/home/me/proj/");
    assert_eq!(args[args.len() - 1], "nerve-server:/root/nerve-repos/proj/");
}

#[test]
fn rsync_args_normalizes_trailing_slash_on_source() {
    let args = rsync_args("/home/me/proj/", "h", "/r");
    assert_eq!(args[args.len() - 2], "/home/me/proj/");
}

#[test]
fn ssh_args_build_expected_command() {
    let args = ssh_args("nerve-server", &["--jobs", "--json"]);
    assert_eq!(
        args,
        vec![
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "nerve-server",
            "nerve",
            "--jobs",
            "--json",
        ]
    );
}
