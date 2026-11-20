//! judges-see-the-check.test.tsx, "your check, in your benchmark" and the step words of "the benchmark, as Colin's
//! screenshot of 10/21 found it" (0.810): each model's runs whose copy passed the check, the note that says so only
//! when a run was checked, and a run again said by the batch's own count.
use super::*;
use crate::compare::{RerunProgress, RerunStep};

fn answer(key: &str, check: Option<BenchmarkCheck>) -> BenchmarkAnswer {
    BenchmarkAnswer {
        key: key.into(),
        runtime: "opencode".into(),
        model: key.into(),
        effort: None,
        name: key.into(),
        answered: true,
        met: Some(1),
        of: Some(1),
        span_ms: None,
        cost: None,
        check,
        slot: None,
    }
}

fn run(id: &str, answers: Vec<BenchmarkAnswer>) -> BenchmarkRunInput {
    BenchmarkRunInput { compare_id: id.into(), answers }
}

// case: counts the runs each model's copy passed it
#[test]
fn counts_the_runs_each_models_copy_passed_it() {
    use BenchmarkCheck::*;
    let rows = benchmark_rows(&[
        run("cmp_1", vec![answer("haiku", Some(Passed)), answer("sonnet", Some(Failed))]),
        run("cmp_2", vec![answer("haiku", Some(Passed)), answer("sonnet", Some(Passed))]),
    ]);
    let of = |key: &str| rows.iter().find(|r| r.key == key).map(|r| (r.checked, r.check_passed));
    assert_eq!(of("haiku"), Some((2, 2)));
    assert_eq!(of("sonnet"), Some((2, 1)));
    assert_eq!(
        benchmark_note(2, true),
        "A score is the judge's points met, out of 100, averaged over the 2 runs it scored. Met every point counts the runs that met all of them. Check passed counts the runs whose copy passed your check. Time and cost are each run's own, as its agent reported them."
    );
}

// case: has no such column where no run was checked
#[test]
fn has_no_such_column_where_no_run_was_checked() {
    let rows = benchmark_rows(&[run("cmp_1", vec![answer("haiku", None), answer("sonnet", None)])]);
    assert!(rows.iter().all(|r| r.checked == 0));
    let note = benchmark_note(1, false);
    assert!(!note.contains("Check passed"), "{note}");
    assert!(note.contains("averaged over the run it scored."), "{note}");
}

// case: says a run again by the batch's own count, and three judges as three
#[test]
fn says_a_run_again_by_the_batchs_own_count_and_three_judges_as_three() {
    let progress = |run: usize, of: usize, step| RerunProgress { origin_id: "cmp_1".into(), run, of, step, compare_id: None };
    assert_eq!(step_words(&progress(1, 1, RerunStep::Judging), true), "Running it once more: the judges are reading the answers");
    assert_eq!(step_words(&progress(2, 3, RerunStep::Answering), false), "Running it again, 2 of 3: the models are answering");
    assert_eq!(step_words(&progress(1, 1, RerunStep::Judging), false), "Running it once more: the judge is reading the answers");
}
