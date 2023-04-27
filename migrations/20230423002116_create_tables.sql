-- Add migration script here
CREATE TABLE IF NOT EXISTS Users (
    username VARCHAR(255) UNIQUE,
    user_id SERIAL PRIMARY KEY,
    salt VARCHAR(255),
    password_digest VARCHAR(255)
);

CREATE TABLE IF NOT EXISTS Jobs (
    job_id SERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    website VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    budget NUMERIC NULL,
    hourly NUMERIC NULL,
    post_url VARCHAR(255) NOT NULL
);

CREATE TABLE IF NOT EXISTS Proposals (
    proposal_id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES Users(user_id) NOT NULL,
    job_id INTEGER REFERENCES Jobs(job_id) NOT NULL,
    proposal TEXT
);

CREATE TABLE IF NOT EXISTS DecidedJobs (
    user_id INTEGER REFERENCES Users(user_id),
    job_id INTEGER REFERENCES Jobs(job_id),
    proposal_id INTEGER REFERENCES Proposals(proposal_id),
    accepted BOOLEAN,
    PRIMARY KEY (user_id, job_id)
);

CREATE TABLE IF NOT EXISTS PendingJobs (
    user_id INTEGER REFERENCES Users(user_id),
    job_id INTEGER REFERENCES Jobs(job_id),
    proposal_id INTEGER REFERENCES Proposals(proposal_id),
    PRIMARY KEY (user_id, job_id)
);

CREATE TABLE IF NOT EXISTS KMeansClasses (
    user_id INTEGER REFERENCES Users(user_id),
    kmeans_classes INTEGER[],
    job_ids INTEGER[],
    PRIMARY KEY (user_id)
);
