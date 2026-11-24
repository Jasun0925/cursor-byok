//! Resolves Cursor model selections to configured provider models.
use crate::{
    cursor::protocol::proto::agent::v1 as pb,
    model::{
        parse_token_count, ModelLatency, ModelSpec, ReasoningSpec, SubagentKind,
        SubagentModelOverride,
    },
    Error, Result,
};

pub fn requested_model(request: &pb::AgentRunRequest) -> Result<ModelSpec> {
    let details = request.model_details.as_ref();
    let model = if let Some(requested) = request.requested_model.as_ref() {
        from_requested(requested, details)?
    } else if let Some(model_id) = details
        .map(|model| model.model_id.as_str())
        .filter(|model| !model.is_empty())
    {
        ModelSpec {
            inherit_model_options: true,
            model_id: model_id.into(),
            display_name: details
                .map(|model| model.display_name.clone())
                .filter(|name| !name.is_empty()),
            reasoning: ReasoningSpec {
                enabled: details.is_some_and(|model| model.thinking_details.is_some()),
                effort: None,
            },
            latency: ModelLatency::Standard,
            max_output_tokens: None,
            context_window_tokens: None,
            supports_image_generation: false,
            strip_images: false,
            extra_params: serde_json::json!({}),
        }
    } else {
        return Err(Error::Protocol("Cursor Run does not select a model".into()));
    };
    Ok(model)
}

pub fn overrides(
    request: &pb::AgentRunRequest,
) -> Result<Vec<(SubagentKind, SubagentModelOverride)>> {
    request
        .subagent_model_overrides
        .iter()
        .map(|value| {
            use pb::subagent_model_override::Selection;
            let kind = subagent_kind(&value.subagent_type);
            let selection = match value.selection.as_ref() {
                Some(Selection::Model(model)) => {
                    if model.model_id == "default" {
                        SubagentModelOverride::Inherit
                    } else {
                        SubagentModelOverride::Explicit(from_requested(model, None)?)
                    }
                }
                Some(Selection::Inherit(true)) => SubagentModelOverride::Inherit,
                Some(Selection::Disabled(true)) => SubagentModelOverride::Disabled,
                None | Some(Selection::Inherit(false) | Selection::Disabled(false)) => {
                    return Err(Error::Protocol(format!(
                        "Cursor subagent model override {} has no active selection",
                        value.subagent_type
                    )))
                }
            };
            Ok((kind, selection))
        })
        .collect()
}

pub fn subagent_kind(value: &str) -> SubagentKind {
    if value == "generalPurpose" {
        SubagentKind::GeneralPurpose
    } else {
        SubagentKind::Named(value.into())
    }
}

fn from_requested(
    model: &pb::RequestedModel,
    details: Option<&pb::ModelDetails>,
) -> Result<ModelSpec> {
    let mut spec = ModelSpec {
        inherit_model_options: true,
        model_id: model.model_id.clone(),
        display_name: details
            .map(|model| model.display_name.clone())
            .filter(|name| !name.is_empty()),
        reasoning: ReasoningSpec {
            enabled: model.max_mode
                || details.is_some_and(|model| model.thinking_details.is_some()),
            effort: None,
        },
        latency: ModelLatency::Standard,
        max_output_tokens: None,
        context_window_tokens: None,
        supports_image_generation: false,
        strip_images: false,
        extra_params: serde_json::json!({}),
    };
    for parameter in &model.parameters {
        match parameter.id.as_str() {
            "effort" | "reasoning" => {
                let effort = parameter.value.trim();
                spec.reasoning.effort =
                    (effort != "none" && !effort.is_empty()).then(|| effort.to_string());
                spec.reasoning.enabled |= spec.reasoning.effort.is_some();
            }
            "thinking" => spec.reasoning.enabled |= parse_bool(parameter)?,
            "fast" => {
                if parse_bool(parameter)? {
                    spec.latency = ModelLatency::Fast;
                }
            }
            "context" => {
                spec.context_window_tokens =
                    Some(parse_token_count(&parameter.value).ok_or_else(|| {
                        Error::Protocol(format!(
                            "invalid Cursor context token count: {}",
                            parameter.value
                        ))
                    })?);
            }
            _ => {}
        }
    }
    Ok(spec)
}

fn parse_bool(parameter: &pb::requested_model::ModelParameterValue) -> Result<bool> {
    match parameter.value.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(Error::Protocol(format!(
            "invalid Cursor boolean model parameter {}={}",
            parameter.id, parameter.value
        ))),
    }
}

/// Projects only model identity into the append-only context, never credentials.
/// Keep the Task schema stable when the client's selected models change.
pub(super) fn subagent_model_context(
    request: &pb::AgentRunRequest,
    local_model_names: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut models = std::collections::BTreeMap::<&str, &str>::new();
    for model in &request.selected_subagent_models {
        if !model.model_id.is_empty() {
            models.entry(&model.model_id).or_default();
        }
    }
    for details in &request.selected_subagent_model_details {
        if !details.model_id.is_empty() {
            models
                .entry(&details.model_id)
                .and_modify(|name| {
                    if !details.display_name.is_empty()
                        && (name.is_empty() || details.display_name.as_str() < *name)
                    {
                        *name = &details.display_name;
                    }
                })
                .or_insert(&details.display_name);
        }
    }
    // Cursor sends local model hashes without labels. Resolve only identities
    // present in this request; never advertise unrelated local configurations.
    for (id, name) in &mut models {
        if let Some(local_name) = local_model_names.get(*id).filter(|name| !name.is_empty()) {
            *name = local_name;
        }
    }
    // Match exec_context: the first override supplies the default Task model.
    // Do not advertise per-type routing that the runtime does not implement.
    let configured = match request
        .subagent_model_overrides
        .first()
        .and_then(|value| value.selection.as_ref())
    {
        Some(pb::subagent_model_override::Selection::Model(model))
            if model.model_id != "default" =>
        {
            serde_json::json!({
                "mode":"configured", "model_id":model.model_id,
                "display_name": local_model_names.get(&model.model_id).map(String::as_str)
                    .or_else(|| models.get(model.model_id.as_str()).copied()).unwrap_or_default()
            })
        }
        Some(pb::subagent_model_override::Selection::Model(_))
        | Some(pb::subagent_model_override::Selection::Inherit(true)) => {
            serde_json::json!({"mode":"inherit_parent"})
        }
        Some(pb::subagent_model_override::Selection::Disabled(true)) => {
            serde_json::json!({"mode":"disabled"})
        }
        _ => serde_json::json!({"mode":"no_override"}),
    };
    let data = serde_json::json!({"client_selected_models":models, "cursor_setting":configured});
    // Escape markup delimiters in client labels without serializing credential fields.
    let data = data
        .to_string()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("<subagent_models>\nCursor-supplied model IDs mapped to display names (local BYOK names resolve local IDs), and the effective Cursor subagent setting. An empty map means no model list was supplied, not that subagents are unavailable. An empty display name means the label is unknown, not that the model is unavailable. When the user names a model, match its display name and pass the corresponding map key as Task.model, never a guessed ID or display name. Use inherit by default. For a new subagent, an explicit Task.model takes precedence over the Cursor setting. If Task.model is omitted or inherit, use the configured Cursor model, or the parent model for inherit_parent/no_override. The disabled setting always prevents launching subagents. A different configured model is a default, not a conflict; honor the user's explicitly requested model without silently substituting another model. This latest selection policy supersedes older model-selection instructions in the conversation.\n{data}\n</subagent_models>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_unknown_cursor_model_parameters() {
        let requested = pb::RequestedModel {
            model_id: "test-model".into(),
            parameters: vec![pb::requested_model::ModelParameterValue {
                id: "optimize_for".into(),
                value: "quality".into(),
            }],
            ..Default::default()
        };

        let model = from_requested(&requested, None).expect("unknown parameter should be ignored");

        assert_eq!(model.model_id, "test-model");
        assert_eq!(model.latency, ModelLatency::Standard);
        assert!(!model.reasoning.enabled);
    }
}
