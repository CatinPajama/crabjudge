-- Add migration script here
CREATE TABLE verification (
    id BIGSERIAL PRIMARY KEY,
    email TEXT NOT NULL,
    token_type TEXT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (email, token_type)
);
