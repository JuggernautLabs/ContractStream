DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'shane') THEN
        CREATE ROLE shane WITH LOGIN PASSWORD 'isGod';
    END IF;
END
$$;