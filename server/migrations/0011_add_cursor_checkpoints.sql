CREATE TABLE cursor_checkpoints (
    checkpoint_id INTEGER PRIMARY KEY,
    state BLOB NOT NULL,
    FOREIGN KEY (checkpoint_id) REFERENCES conversation_checkpoints(checkpoint_id) ON DELETE CASCADE
);
