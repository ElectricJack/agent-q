//! The agent's `task.create` body (`POST /v1/agent/tasks`, spec 0006 phase 2)
//! and the single-task Graph action it becomes. aq-server checks the grant,
//! reserves the quota and submits the action under the attempt's identity.
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTask {
    pub api_version: u32,
    pub operation_id: Uuid,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub acceptance_criteria: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub parent_id: Option<Value>,
    pub priority: Option<u16>,
    pub task_type: Option<String>,
    pub task_state: Option<String>,
    pub expected_project_revision: Option<String>,
    pub profile_hint: Option<Value>,
    pub intelligence_hint: Option<Value>,
}
impl AgentTask {
    /// §8: an agent cannot pick the profile or intelligence of a task it
    /// creates, so it cannot launch a child with more authority than its own.
    pub fn sets_hint(&self) -> bool {
        [&self.profile_hint, &self.intelligence_hint]
            .iter()
            .any(|h| h.as_ref().is_some_and(|v| !v.is_null()))
    }
    /// The parent: `"self"` is the attempt's task `own`; none is top-level.
    pub fn parent(&self, own: Uuid) -> Option<Uuid> {
        match &self.parent_id {
            Some(Value::String(s)) if s == "self" => Some(own),
            Some(v) if !v.is_null() => v.as_str().and_then(|s| s.parse().ok()),
            _ => None,
        }
    }
    /// The `task.create` action request on `project` at `revision`; tasks
    /// are `active` unless the agent asked for a draft.
    pub fn action(&self, project: Uuid, own: Uuid, revision: &str) -> Value {
        let payload = json!({"operation_id":self.operation_id,"expected_project_revision":revision,"title":self.title,
            "body":self.body,"acceptance_criteria":self.acceptance_criteria,"labels":self.labels,
            "task_state":self.task_state.as_deref().unwrap_or("active"),"parent_id":self.parent(own),
            "priority":self.priority,"task_type":self.task_type});
        json!({"operation_id":self.operation_id,"kind":"task.create","subject":{"type":"project","id":project,"revision":revision},
            "expected_versions":{"project":revision},"payload":payload})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_agent_task_becomes_one_create_action_without_hints() {
        let (own, project, op) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
        let t: AgentTask = serde_json::from_value(
            json!({"api_version":1,"operation_id":op,"title":"x","parent_id":"self","profile_hint":null}),
        )
        .unwrap();
        assert!(!t.sets_hint());
        assert_eq!(t.parent(own), Some(own));
        let a = t.action(project, own, "4");
        assert_eq!(
            a["subject"],
            json!({"type":"project","id":project,"revision":"4"})
        );
        assert_eq!(a["payload"]["task_state"], "active");
        assert_eq!(a["payload"]["parent_id"], json!(own));
        let hinted: AgentTask = serde_json::from_value(
            json!({"api_version":1,"operation_id":op,"title":"x","intelligence_hint":"max"}),
        )
        .unwrap();
        assert!(hinted.sets_hint() && hinted.parent(own).is_none());
        assert!(
            serde_json::from_value::<AgentTask>(
                json!({"api_version":1,"operation_id":op,"title":"x","extra":1})
            )
            .is_err()
        );
    }
}
