//! Narrowing the cached task list for display.
//!
//! Deliberately free of both the clock and the database: `today` arrives as a
//! token so every rule is testable without either, and the caller decides what
//! "today" means. That matters for due dates, which are a local-calendar
//! concept -- a task due today should not read as overdue because the machine
//! is a few hours ahead in UTC.

use crate::config::ViewConfig;
use crate::nextcloud::{Task, TaskStatus};

/// The date portion of an iCalendar DUE value: `20260622T140000Z` -> `20260622`.
/// Returns `None` for anything that isn't at least eight leading digits, so a
/// malformed value is treated as "no usable date" rather than matched by luck.
fn due_date(token: &str) -> Option<&str> {
    let token = token.trim();
    if token.len() < 8 || !token.as_bytes()[..8].iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(&token[..8])
}

/// Today as the same `YYYYMMDD` token shape a DUE value carries, in local time.
pub fn today_token() -> String {
    chrono::Local::now().format("%Y%m%d").to_string()
}

/// Whether one task's DUE value satisfies the active due filter.
///
/// `on` (an exact date, set by tapping a due pill) takes precedence over `rule`
/// (a preset) because it is the more specific of the two; the setter keeps them
/// mutually exclusive, and this ordering means a stale `rule` can never quietly
/// override an explicit tap.
pub fn due_matches(due: Option<&str>, rule: &str, on: &str, today: &str) -> bool {
    if !on.is_empty() {
        return due.and_then(due_date) == Some(on);
    }
    match rule {
        "" => true,
        "has" => due.is_some(),
        "none" => due.is_none(),
        // Date tokens are fixed-width and zero-padded, so lexical order is
        // chronological order -- no parsing required to compare them.
        "overdue" => matches!(due.and_then(due_date), Some(date) if date < today),
        "today" => matches!(due.and_then(due_date), Some(date) if date == today),
        // An unrecognized rule must not silently empty the list: a config typo
        // should look like "no filter", not like "everything is gone".
        _ => true,
    }
}

/// Apply the persisted view to a task list.
///
/// `include_completed` widens rather than narrows, which is why it is not part
/// of [`ViewConfig::is_filtered`]: showing finished tasks is not a state anyone
/// needs warning about.
pub fn apply(tasks: Vec<Task>, view: &ViewConfig, today: &str) -> Vec<Task> {
    tasks
        .into_iter()
        .filter(|task| {
            view.include_completed
                || !matches!(task.status, TaskStatus::Completed | TaskStatus::Cancelled)
        })
        .filter(|task| due_matches(task.due.as_deref(), &view.due, &view.due_on, today))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(uid: &str, due: Option<&str>, status: TaskStatus) -> Task {
        Task {
            uid: uid.to_string(),
            summary: uid.to_string(),
            status,
            due: due.map(str::to_string),
        }
    }

    const TODAY: &str = "20260911";

    #[test]
    fn due_presets_classify_against_the_supplied_day() {
        assert!(due_matches(Some("20260910"), "overdue", "", TODAY));
        assert!(!due_matches(Some("20260911"), "overdue", "", TODAY));
        assert!(!due_matches(Some("20260912"), "overdue", "", TODAY));

        assert!(due_matches(Some("20260911"), "today", "", TODAY));
        // A timed value on today's date is still due today: only the date part
        // participates, so 09:00 does not become "overdue" by lunchtime.
        assert!(due_matches(Some("20260911T090000"), "today", "", TODAY));
        assert!(!due_matches(Some("20260911T090000"), "overdue", "", TODAY));

        assert!(due_matches(Some("20260101"), "has", "", TODAY));
        assert!(!due_matches(None, "has", "", TODAY));
        assert!(due_matches(None, "none", "", TODAY));
        assert!(!due_matches(Some("20260101"), "none", "", TODAY));
    }

    #[test]
    fn an_exact_date_wins_over_a_preset() {
        // Tapping a due pill is more specific than a preset, and must not be
        // overridden by one that happens to still be set.
        assert!(due_matches(
            Some("20261224T180000Z"),
            "overdue",
            "20261224",
            TODAY
        ));
        assert!(!due_matches(Some("20261225"), "overdue", "20261224", TODAY));
    }

    #[test]
    fn no_filter_and_unknown_filters_both_pass_everything() {
        assert!(due_matches(None, "", "", TODAY));
        assert!(due_matches(Some("20260101"), "", "", TODAY));
        // A typo in config.toml should degrade to "no filter", never to an
        // empty list the user cannot explain.
        assert!(due_matches(None, "someday-maybe", "", TODAY));
        assert!(due_matches(Some("20260101"), "someday-maybe", "", TODAY));
    }

    #[test]
    fn malformed_due_values_never_match_a_date_rule() {
        assert!(!due_matches(Some("nonsense"), "overdue", "", TODAY));
        assert!(!due_matches(Some("2026"), "today", "", TODAY));
        // ... but they still count as "has a due date", because one is present.
        assert!(due_matches(Some("nonsense"), "has", "", TODAY));
    }

    #[test]
    fn completed_tasks_are_hidden_unless_explicitly_included() {
        let tasks = vec![
            task("open", Some("20260910"), TaskStatus::NeedsAction),
            task("done", Some("20260910"), TaskStatus::Completed),
            task("cancelled", Some("20260910"), TaskStatus::Cancelled),
        ];
        let mut view = ViewConfig::default();
        assert_eq!(apply(tasks.clone(), &view, TODAY).len(), 1);
        view.include_completed = true;
        assert_eq!(apply(tasks, &view, TODAY).len(), 3);
    }

    #[test]
    fn filters_compose_rather_than_replace_each_other() {
        let tasks = vec![
            task("overdue-open", Some("20260910"), TaskStatus::NeedsAction),
            task("overdue-done", Some("20260910"), TaskStatus::Completed),
            task("future-open", Some("20261001"), TaskStatus::NeedsAction),
            task("undated-open", None, TaskStatus::NeedsAction),
        ];
        let view = ViewConfig {
            include_completed: false,
            due: "overdue".to_string(),
            due_on: String::new(),
        };
        let kept = apply(tasks, &view, TODAY);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].uid, "overdue-open");
    }

    #[test]
    fn is_filtered_ignores_the_widening_toggle() {
        let mut view = ViewConfig::default();
        assert!(!view.is_filtered());
        // Showing completed tasks makes the list longer, not shorter -- nobody
        // needs to be warned that they can see more.
        view.include_completed = true;
        assert!(!view.is_filtered());
        view.due = "overdue".to_string();
        assert!(view.is_filtered());
    }
}
