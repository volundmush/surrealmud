DEFINE TABLE OVERWRITE user SCHEMALESS;
DEFINE FIELD OVERWRITE email ON TABLE user TYPE string VALUE string::trim(string::lowercase($value)) ASSERT string::is::email($value);
DEFINE FIELD OVERWRITE password ON TABLE user TYPE string VALUE crypto::argon2::generate(string::trim($value));
DEFINE FIELD OVERWRITE time_created ON TABLE user TYPE datetime DEFAULT time::now() READONLY;

DEFINE INDEX OVERWRITE unique_email ON TABLE user FIELDS email UNIQUE;
DEFINE ACCESS OVERWRITE account ON DATABASE TYPE RECORD
    SIGNUP ( CREATE user SET email = $email, password = $password)
    SIGNIN ( SELECT * FROM user WHERE email = $email.trim().lowercase() AND crypto::argon2::compare(password, $password.trim()) );

DEFINE TABLE OVERWRITE conn SCHEMALESS
    PERMISSIONS
        FOR create
            WHERE record::exists($auth)
        FOR select
            WHERE user = $auth.id
        FOR update, delete
            WHERE user = $auth.id
;

DEFINE FIELD OVERWRITE user ON TABLE conn TYPE record<user> READONLY;
DEFINE FIELD OVERWRITE time_created ON TABLE conn TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE time_activity ON TABLE conn TYPE datetime DEFAULT time::now() READONLY;

