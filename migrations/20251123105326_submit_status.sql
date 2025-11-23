-- Add migration script here

CREATE TABLE submit_status(
    submission_id BIGSERIAL PRIMARY KEY,
    user_id BIGINT references users(user_id) NOT NULL,
    problem_id BIGINT references problems(problem_id) NOT NULL, 
    status text NOT NULL,
    output TEXT NOT NULL
);
