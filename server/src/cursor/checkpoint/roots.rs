//! Maintains stable append-only Cursor root messages.
use crate::{
    cursor::checkpoint::messages,
    model::{CanonicalMessage, ContentPart, MessageContent},
    store::BlobId,
    Error, Result,
};

use super::CheckpointBuilder;

#[derive(Clone)]
pub(super) struct RootFrontier {
    pub(super) ids: Vec<BlobId>,
    pub(super) generated: Vec<Vec<u8>>,
    pub(super) base_count: usize,
}

impl CheckpointBuilder {
    pub(super) async fn project_roots(
        &mut self,
        messages: &[CanonicalMessage],
    ) -> Result<Vec<BlobId>> {
        let messages = attach_stored_tool_images(&self.store, messages).await?;
        let wire_messages = messages::stable_messages(&self.instructions, &messages, &self.model)?;
        self.ensure_roots()?;
        let replacement = self
            .roots
            .as_ref()
            .and_then(|roots| changed_system_root(roots, &wire_messages));
        if let Some(message) = replacement {
            let id = self.sync.persist(&message, &[]).await?;
            self.roots
                .as_mut()
                .ok_or_else(|| Error::Protocol("Cursor root frontier was not initialized".into()))?
                .ids[0] = id;
        }
        let roots = self
            .roots
            .as_mut()
            .ok_or_else(|| Error::Protocol("Cursor root frontier was not initialized".into()))?;
        if wire_messages.len() < roots.ids.len() {
            return Err(Error::Protocol(format!(
                "Cursor stable history shrank from {} to {} roots",
                roots.ids.len(),
                wire_messages.len()
            )));
        }
        for (index, expected) in roots.generated.iter().enumerate() {
            let wire_index = roots.base_count + index;
            if wire_messages.get(wire_index) != Some(expected) {
                return Err(Error::Protocol(format!(
                    "Cursor stable root changed at index {wire_index}"
                )));
            }
        }
        for message in wire_messages.iter().skip(roots.ids.len()) {
            roots.ids.push(self.sync.persist(message, &[]).await?);
            roots.generated.push(message.clone());
        }
        Ok(roots.ids.clone())
    }

    fn ensure_roots(&mut self) -> Result<()> {
        if self.roots.is_some() {
            return Ok(());
        }
        let ids = self
            .base
            .root_prompt_messages_json
            .iter()
            .map(|id| BlobId::from_bytes(id))
            .collect::<Result<Vec<_>>>()?;
        self.roots = Some(RootFrontier {
            base_count: ids.len(),
            ids,
            generated: Vec::new(),
        });
        Ok(())
    }

    pub(super) async fn replace_roots(
        &mut self,
        messages: &[CanonicalMessage],
    ) -> Result<Vec<BlobId>> {
        let messages = attach_stored_tool_images(&self.store, messages).await?;
        let wire_messages = messages::stable_messages(&self.instructions, &messages, &self.model)?;
        self.ensure_roots()?;
        let previous_system = self
            .roots
            .as_ref()
            .and_then(|roots| roots.ids.first())
            .cloned();
        let mut ids = Vec::with_capacity(wire_messages.len());
        for (index, message) in wire_messages.iter().enumerate() {
            if index == 0
                && previous_system
                    .as_ref()
                    .is_some_and(|id| *id == BlobId::digest(message))
            {
                ids.push(previous_system.clone().expect("checked system root"));
            } else {
                ids.push(self.sync.persist(message, &[]).await?);
            }
        }
        self.roots = Some(RootFrontier {
            base_count: ids.len(),
            ids: ids.clone(),
            generated: Vec::new(),
        });
        Ok(ids)
    }
}

async fn attach_stored_tool_images(
    store: &crate::store::Store,
    messages: &[CanonicalMessage],
) -> Result<Vec<CanonicalMessage>> {
    let mut messages = messages.to_vec();
    for message in &mut messages {
        let MessageContent::ToolResult(result) = &mut message.content else {
            continue;
        };
        if result
            .provider_parts
            .iter()
            .any(|part| matches!(part, ContentPart::Image { .. }))
        {
            continue;
        }
        let Some(image) = result.image.clone() else {
            continue;
        };
        let id = BlobId::from_base64(&image.blob_id)?;
        let data = store.get_blob(&id).await?.ok_or_else(|| {
            Error::Protocol(format!("Read image Blob is missing: {}", image.blob_id))
        })?;
        let text = result.content.clone();
        result.provider_parts = vec![
            ContentPart::Text { text },
            ContentPart::Image {
                mime_type: image.mime_type,
                data,
            },
        ];
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{MessageContent, Origin, Role, ToolImageReference, ToolResultContent},
        store::Store,
    };

    #[tokio::test]
    async fn stored_read_image_is_embedded_before_checkpoint_encoding() {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        let blob_id = store.put_blob(&[9, 8, 7], &[]).await.unwrap();
        let message = CanonicalMessage {
            message_id: "result".into(),
            role: Role::Tool,
            origin: Origin::Tool,
            runtime_event_id: None,
            content: MessageContent::ToolResult(ToolResultContent {
                call_id: "call-1".into(),
                name: "Read".into(),
                content: "Read image file: /tmp/shot.png".into(),
                is_error: false,
                image: Some(ToolImageReference {
                    blob_id: blob_id.to_base64(),
                    mime_type: "image/png".into(),
                    path: "/tmp/shot.png".into(),
                }),
                provider_parts: Vec::new(),
            }),
        };
        let prepared = attach_stored_tool_images(&store, &[message]).await.unwrap();
        let wire = messages::stable_messages("", &prepared, "model").unwrap();
        let decoded = messages::decode(&wire[0], "cursor-root:stored".into()).unwrap();
        let MessageContent::ToolResult(result) = decoded.content else {
            panic!("expected tool result");
        };
        assert_eq!(
            result.provider_parts,
            vec![
                ContentPart::Text {
                    text: "Read image file: /tmp/shot.png".into(),
                },
                ContentPart::Image {
                    mime_type: "image/png".into(),
                    data: vec![9, 8, 7],
                },
            ]
        );
    }
}

fn changed_system_root(roots: &RootFrontier, messages: &[Vec<u8>]) -> Option<Vec<u8>> {
    roots
        .ids
        .first()
        .zip(messages.first())
        .filter(|(current, message)| **current != BlobId::digest(message))
        .map(|(_, message)| message.clone())
}
