-- Add migration script here

CREATE TABLE problems (
    problem_id BIGSERIAL PRIMARY KEY,
    statement TEXT NOT NULL
);

CREATE TABLE problem_testcases (
    problem_id BIGINT references problems(problem_id),
    testcase TEXT NOT NULL,
    output TEXT NOT NULL
);

