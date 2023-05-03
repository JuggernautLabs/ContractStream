
-- Add migration script here

DROP TABLE CareerInfo CASCADE;
CREATE TABLE IF NOT EXISTS CareerInfo (
    info_id SERIAL PRIMARY KEY,
    job_resume TEXT default "" NOT NULL,
    keywords varchar(255)[] default [] NOT NULL,
    user_id INTEGER REFERENCES Users(user_id) NOT NULL,
    job_id INTEGER REFERENCES Jobs(job_id) NOT NULL
);