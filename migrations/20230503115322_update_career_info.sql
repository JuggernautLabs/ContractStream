
-- Add migration script here
CREATE TABLE IF NOT EXISTS Resumes (
    resume_id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES Users(user_id) NOT NULL,
    resume_text TEXT NOT NULL
);


DROP TABLE CareerInfo CASCADE;
CREATE TABLE IF NOT EXISTS SearchContext (
    context_id SERIAL PRIMARY KEY,
    resume_id INTEGER REFERENCES Resumes(resume_id) NOT NULL,
    keywords varchar(255)[] default ARRAY[]::varchar[] NOT NULL,
    user_id INTEGER REFERENCES Users(user_id) NOT NULL
);

