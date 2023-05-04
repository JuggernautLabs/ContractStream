-- Add migration script here
CREATE OR REPLACE FUNCTION insert_user(p_username VARCHAR, p_password VARCHAR)
RETURNS INTEGER AS $$
DECLARE
  v_user_id INTEGER;
BEGIN
  INSERT INTO Users (username, password_digest)
  VALUES (p_username, crypt(p_password, gen_salt('bf')))
  RETURNING user_id INTO v_user_id;

  RETURN v_user_id;
END;
$$ LANGUAGE plpgsql;