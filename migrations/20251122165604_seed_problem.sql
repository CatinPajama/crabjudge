-- Add migration script here

INSERT INTO problems (statement)
VALUES
  ('Given T integers, print "EVEN" if the number is even, otherwise print "ODD" for each number.');

INSERT INTO problem_testcases (problem_id, testcase, output)
VALUES
  (1, '3 4 7 10', 'EVEN ODD EVEN');
