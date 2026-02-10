-- Add migration script here
ALTER table submit_status alter column output drop NOT NULL, alter column status set default 'PENDING';
