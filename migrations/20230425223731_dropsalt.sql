-- Add migration script here
DROP table Users CASCADE;
CREATE TABLE IF NOT EXISTS Users (
    username VARCHAR(255) UNIQUE,
    user_id SERIAL PRIMARY KEY,
    password_digest VARCHAR(255)
);