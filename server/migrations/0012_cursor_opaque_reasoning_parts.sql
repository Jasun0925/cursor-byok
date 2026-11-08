-- Cursor opaque replay state changes from one signature string to the list of
-- reasoning parts `[{"signature", "text"}]`, so messages whose reasoning Cursor
-- signed part by part can be stored. Keys are written in serde_json's sorted
-- order so stored payloads stay byte-identical to a fresh serialization.
UPDATE messages
SET payload_json = json_set(
    payload_json,
    '$.content.replay_state.value',
    json_array(json_object(
        'signature', json_extract(payload_json, '$.content.replay_state.value'),
        'text', json_extract(payload_json, '$.content.thinking')
    ))
)
WHERE json_extract(payload_json, '$.content.replay_state.provider_kind') = 'cursor_opaque'
  AND json_type(payload_json, '$.content.replay_state.value') = 'text';

UPDATE tool_rounds
SET assistant_json = json_set(
    assistant_json,
    '$.replay_state.value',
    json_array(json_object(
        'signature', json_extract(assistant_json, '$.replay_state.value'),
        'text', json_extract(assistant_json, '$.thinking')
    ))
)
WHERE json_extract(assistant_json, '$.replay_state.provider_kind') = 'cursor_opaque'
  AND json_type(assistant_json, '$.replay_state.value') = 'text';
