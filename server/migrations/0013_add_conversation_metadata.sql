-- Metadata may arrive before RunSSE, so it has no checkpoint or conversation FK.
CREATE TABLE conversation_metadata (
    conversation_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(conversation_id)) > 0),
    name TEXT,
    workspace_paths_json TEXT NOT NULL CHECK (json_valid(workspace_paths_json) AND json_type(workspace_paths_json) = 'array')
);
