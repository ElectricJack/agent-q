//! Exact stored subject dispatch. Old code-review bodies/AAD remain unchanged.
use super::*;
use serde::Serializer;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Profile {
    Code,
    SourceAndEffects,
}
impl Profile {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Code => "code_review_v1",
            Self::SourceAndEffects => "source_and_effects_review_v1",
        }
    }
}
#[derive(Clone)]
pub(super) enum Subject {
    Code(ReviewSubject),
    SourceAndEffects(combined::CombinedSubject),
}
impl std::ops::Deref for Subject {
    type Target = ReviewSubject;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Code(s) => s,
            Self::SourceAndEffects(s) => &s.code,
        }
    }
}
impl Serialize for Subject {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Code(v) => v.serialize(s),
            Self::SourceAndEffects(v) => v.serialize(s),
        }
    }
}
impl Subject {
    pub(super) fn artifact(&self) -> Result<ArtifactRef> {
        match self {
            Self::Code(v) => v.artifact().map_err(|_| invalid()),
            Self::SourceAndEffects(v) => v.artifact(),
        }
    }
    pub(super) fn profile(&self) -> Profile {
        match self {
            Self::Code(_) => Profile::Code,
            Self::SourceAndEffects(_) => Profile::SourceAndEffects,
        }
    }
    pub(super) fn may_approve(&self) -> bool {
        match self {
            Self::Code(_) => true,
            Self::SourceAndEffects(v) => v
                .external
                .known_effects
                .iter()
                .all(|e| e.disposition == combined::ExternalDisposition::Reconciled),
        }
    }
}
impl Service {
    pub(super) fn decode_subject(
        &self,
        digest: &str,
        row: &sqlx::postgres::PgRow,
    ) -> Result<Subject> {
        match row
            .try_get::<String, _>("subject_profile")
            .map_err(db)?
            .as_str()
        {
            "code_review_v1" => Ok(Subject::Code(self.decode("subject", digest, row)?)),
            "source_and_effects_review_v1" => Ok(Subject::SourceAndEffects(self.decode(
                "combined-subject",
                digest,
                row,
            )?)),
            _ => Err(AppError::Internal),
        }
    }
}
