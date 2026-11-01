//! Exact typed definitions, never capability-string inference or plugin hooks.
use super::changed;
use crate::{AppState, error::AppError, profiled_execution::registration::RegisteredRuntime};
use aq_admission::{
    ArtifactRef,
    scoped_presence::{QUALIFICATION_PREFIX, Qualification},
};
#[derive(Clone)]
pub(super) struct Loaded {
    pub reference: ArtifactRef,
    pub body: Qualification,
}
impl Loaded {
    pub fn load(state: &AppState, reference: &ArtifactRef) -> Result<Self, AppError> {
        if reference
            .id
            .as_str()
            .strip_prefix(QUALIFICATION_PREFIX)
            .is_none_or(|s| s.is_empty())
        {
            return Err(changed());
        }
        let cfg = state
            .config
            .profiled_execution
            .as_ref()
            .ok_or_else(changed)?;
        let exact = |r: &ArtifactRef| {
            cfg.definition_bodies
                .iter()
                .find(|d| d.artifact() == r)
                .ok_or_else(changed)
        };
        let definition = exact(reference)?;
        if definition.body().len() > 8192 {
            return Err(changed());
        }
        let body: Qualification = serde_json::from_str(definition.body()).map_err(|_| changed())?;
        body.validate().map_err(|_| changed())?;
        for r in [
            &body.runtime,
            &body.runtime_qualification,
            &body.adapter,
            &body.adapter_qualification,
        ] {
            exact(r)?;
        }
        Ok(Self {
            reference: reference.clone(),
            body,
        })
    }
    pub fn matches_runtime(&self, runtime: &RegisteredRuntime) -> Result<(), AppError> {
        if self.body.runtime != runtime.runtime
            || self.body.runtime_qualification != runtime.qualification
        {
            return Err(changed());
        }
        Ok(())
    }
}
