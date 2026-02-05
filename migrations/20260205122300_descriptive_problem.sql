-- Add migration script here
SET LOCAL lock_timeout = '2s';
ALTER TABLE problems 
ADD title TEXT NOT NULL DEFAULT 'Untitled',
ADD difficulty TEXT NOT NULL DEFAULT 'easy';
