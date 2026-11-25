//! Wizard open/resolve transitions, kept free of window handles so they are
//! unit-testable.

use kernel::templates::TemplateJobs;

use super::NewProjectState;

pub fn reset_for_open(state: &mut NewProjectState, jobs: &mut TemplateJobs) {
    *state = NewProjectState::default();
    state.visible = true;
    // A stale result would otherwise close the wizard (success) or show an
    // old error (failure) the moment it reopens.
    jobs.last_result = None;
}

/// Consume a finished generation: success closes the wizard, failure keeps
/// it open with the error.  The result is taken so it is applied once.
pub fn resolve_generation(state: &mut NewProjectState, jobs: &mut TemplateJobs) {
    if jobs.is_generating() {
        return;
    }
    match jobs.last_result.take() {
        Some(Ok(_)) => state.visible = false,
        Some(Err(e)) => state.note = e,
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::templates::GenerationSummary;

    fn ok_result() -> Result<GenerationSummary, String> {
        Ok(GenerationSummary {
            template_id: "t".into(),
            name: "n".into(),
            dest: "/tmp/n".into(),
            files: vec![],
        })
    }

    #[test]
    fn reopening_after_success_stays_open() {
        let mut state = NewProjectState::default();
        let mut jobs = TemplateJobs::default();
        jobs.last_result = Some(ok_result());
        resolve_generation(&mut state, &mut jobs);
        assert!(!state.visible);

        reset_for_open(&mut state, &mut jobs);
        resolve_generation(&mut state, &mut jobs);
        assert!(state.visible);
    }

    #[test]
    fn failure_is_shown_once_then_cleared() {
        let mut state = NewProjectState::default();
        state.visible = true;
        let mut jobs = TemplateJobs::default();
        jobs.last_result = Some(Err("disk full".into()));
        resolve_generation(&mut state, &mut jobs);
        assert_eq!(state.note, "disk full");
        assert!(state.visible);
        assert!(jobs.last_result.is_none());

        reset_for_open(&mut state, &mut jobs);
        assert!(state.note.is_empty());
    }

    #[test]
    fn in_flight_generation_is_not_resolved() {
        let mut state = NewProjectState::default();
        state.visible = true;
        let mut jobs = TemplateJobs::default();
        jobs.begin("t", "n", "/tmp/n".into(), true);
        jobs.last_result = Some(ok_result());
        resolve_generation(&mut state, &mut jobs);
        assert!(state.visible);
    }
}
