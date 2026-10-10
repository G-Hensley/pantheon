//! Observations, not inferred task transitions. Silence cannot distinguish a
//! thinking model, an approval prompt, or a worker that forgot to report back.
use crate::mcp::Task;
use serde::Serialize;

pub(crate) const QUIET_WARNING_MS: u64 = 5 * 60 * 1000;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TaskActivity {
    pub task_id: String,
    pub actor: String,
    pub state: String,
    pub quiet_ms: Option<u64>,
    pub waiting_on: Option<String>,
}

pub(crate) fn actor(task: &Task) -> &str {
    if task.status == "in_review" {
        &task.reviewer
    } else {
        &task.target
    }
}

pub(crate) fn observe(
    task: &Task,
    output_age_ms: Option<u64>,
    mcp_age_ms: Option<u64>,
    now_ms: u64,
) -> TaskActivity {
    let mut observation = TaskActivity {
        task_id: task.id.clone(),
        actor: actor(task).into(),
        state: task.status.clone(),
        quiet_ms: None,
        waiting_on: None,
    };
    if !task.recovery_reason.is_empty() {
        observation.state = "interrupted".into();
    } else if task.status == "blocked" {
        observation.state = "waiting_on_conductor".into();
        observation.waiting_on = Some(task.from.clone());
    } else if task.status == "queued" {
        // Queued work has not started. Its age is not an inactivity signal.
    } else if task.status == "in_review" && task.reviewer_gone {
        observation.state = "reviewer_unavailable".into();
    } else if task.mode == "headless" && task.status != "in_review" && task.status != "rework" {
        // The interactive pane's output does not describe its headless child.
        observation.state = "headless".into();
    } else if matches!(
        task.status.as_str(),
        "pending" | "overdue" | "rework" | "in_review"
    ) {
        // A recent MCP call is observed activity, not proof of progress. No
        // output clock means we cannot establish a period of silence.
        observation.quiet_ms = output_age_ms.map(|output| {
            output
                .min(mcp_age_ms.unwrap_or(output))
                .min(now_ms.saturating_sub(task.ts_ms))
        });
        if observation
            .quiet_ms
            .is_some_and(|age| age >= QUIET_WARNING_MS)
        {
            observation.state = "quiet".into();
        }
    }
    observation
}

pub(crate) fn describe(observation: &TaskActivity) -> String {
    match observation.state.as_str() {
        "quiet" => format!(
            "task {} on {}: no observed activity for {}s; task still open, reason unknown",
            observation.task_id,
            observation.actor,
            observation.quiet_ms.unwrap_or(0) / 1000
        ),
        "waiting_on_conductor" => format!(
            "task {} on {}: waiting for an answer from {}",
            observation.task_id,
            observation.actor,
            observation.waiting_on.as_deref().unwrap_or("its conductor")
        ),
        "reviewer_unavailable" => format!(
            "task {}: reviewer {} unavailable; reassign review",
            observation.task_id, observation.actor
        ),
        "interrupted" => format!(
            "task {}: assignment interrupted; explicit reassignment required",
            observation.task_id
        ),
        _ => format!("task {}: {}", observation.task_id, observation.state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(status: &str) -> Task {
        serde_json::from_value(serde_json::json!({
            "id": "task-1", "from": "conductor", "target": "worker",
            "task": "Check the parser", "status": status, "result": "",
            "ts_ms": 100, "reviewer": "reviewer"
        }))
        .unwrap()
    }

    #[test]
    fn quiet_requires_the_threshold_and_both_activity_sources() {
        let t = task("pending");
        let now = QUIET_WARNING_MS + 100;
        assert_ne!(
            observe(&t, Some(QUIET_WARNING_MS - 1), None, now).state,
            "quiet"
        );
        assert_eq!(
            observe(&t, Some(QUIET_WARNING_MS), None, now).state,
            "quiet"
        );
        assert_eq!(
            observe(&t, Some(QUIET_WARNING_MS), Some(10), now).quiet_ms,
            Some(10)
        );
        assert_ne!(
            observe(&t, Some(QUIET_WARNING_MS), Some(10), now).state,
            "quiet"
        );
        assert_eq!(
            observe(&t, None, Some(QUIET_WARNING_MS), now).quiet_ms,
            None
        );
        assert_eq!(t.status, "pending");
    }

    #[test]
    fn new_dispatch_and_clock_rollback_cannot_look_quiet() {
        let t = task("pending");
        assert_eq!(observe(&t, Some(900_000), None, 101).quiet_ms, Some(1));
        assert_eq!(observe(&t, Some(900_000), None, 99).quiet_ms, Some(0));
    }

    #[test]
    fn review_activity_belongs_to_reviewer_and_questions_to_conductor() {
        let review = observe(&task("in_review"), Some(100), None, 1000);
        assert_eq!(review.actor, "reviewer");
        let blocked = observe(&task("blocked"), Some(900_000), None, 900_100);
        assert_eq!(blocked.state, "waiting_on_conductor");
        assert_eq!(blocked.waiting_on.as_deref(), Some("conductor"));
        assert!(describe(&blocked).contains("answer from conductor"));
        assert_eq!(blocked.quiet_ms, None);
    }

    #[test]
    fn queued_interrupted_and_headless_work_do_not_borrow_pane_silence() {
        let mut t = task("queued");
        assert_eq!(observe(&t, Some(900_000), None, 900_100).quiet_ms, None);
        t.status = "in_review".into();
        t.recovery_reason = "App restarted".into();
        assert_eq!(
            observe(&t, Some(900_000), None, 900_100).state,
            "interrupted"
        );
        t.recovery_reason.clear();
        t.status = "pending".into();
        t.mode = "headless".into();
        assert_eq!(observe(&t, Some(900_000), None, 900_100).state, "headless");
    }
}
