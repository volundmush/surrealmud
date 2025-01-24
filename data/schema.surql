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
            WHERE record::exists($auth.id)
        FOR select
            WHERE user = $auth.id
        FOR update, delete
            WHERE user = $auth.id
;

DEFINE FIELD OVERWRITE user ON TABLE conn TYPE record<user> READONLY;
DEFINE FIELD OVERWRITE time_created ON TABLE conn TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE time_system_activity ON TABLE conn TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE time_user_activity ON TABLE conn TYPE datetime DEFAULT time::now() READONLY;

DEFINE TABLE OVERWRITE game_session SCHEMALESS
    PERMISSIONS
        FOR create
            WHERE record::exists($auth.id)
        FOR select
            WHERE user = $auth.id
        FOR update, delete
            WHERE user = $auth.id
;

DEFINE FIELD OVERWRITE user ON TABLE game_session TYPE record<user> READONLY;
DEFINE FIELD OVERWRITE character ON TABLE game_session TYPE record REFERENCE ON DELETE IGNORE READONLY;
DEFINE INDEX OVERWRITE unique_character ON TABLE game_session FIELDS character UNIQUE;
DEFINE FIELD OVERWRITE controlling ON TABLE game_session TYPE record REFERENCE ON DELETE IGNORE;
DEFINE FIELD OVERWRITE time_created ON TABLE game_session TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE time_system_activity ON TABLE game_session TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE time_user_activity ON TABLE game_session TYPE datetime DEFAULT time::now() READONLY;

DEFINE TABLE OVERWRITE conn_input SCHEMAFULL
    PERMISSIONS
        FOR create
            WHERE record::exists($auth.id)
        FOR select
            WHERE user = $auth.id
        FOR update, delete
            WHERE user = $auth.id
;

DEFINE FIELD OVERWRITE user ON TABLE conn_input TYPE record<user> READONLY;
DEFINE FIELD OVERWRITE conn ON TABLE conn_input TYPE record<conn> REFERENCE ON DELETE CASCADE READONLY;
DEFINE FIELD OVERWRITE time_created ON TABLE conn_input TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE data_type ON TABLE conn_input TYPE string VALUE $value.trim().lowercase() ASSERT ['command', 'gmcp'].find_index($value) != NONE;
DEFINE FIELD OVERWRITE command ON TABLE conn_input TYPE string READONLY;
DEFINE FIELD OVERWRITE gmcp ON TABLE conn_input TYPE option<object> READONLY;

DEFINE TABLE OVERWRITE conn_output SCHEMAFULL
    PERMISSIONS
        FOR create
            WHERE record::exists($auth.id)
        FOR select
            WHERE user = $auth.id
        FOR update, delete
            WHERE user = $auth.id
;

DEFINE FIELD OVERWRITE user ON TABLE conn_output TYPE record<user> READONLY;
DEFINE FIELD OVERWRITE conn ON TABLE conn_output TYPE record<conn> REFERENCE ON DELETE CASCADE READONLY;
DEFINE FIELD OVERWRITE time_created ON TABLE conn_output TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE data_type ON TABLE conn_output TYPE string VALUE $value.trim().lowercase() ASSERT ['command', 'gmcp'].find_index($value) != NONE;
DEFINE FIELD OVERWRITE command ON TABLE conn_output TYPE string READONLY;
DEFINE FIELD OVERWRITE gmcp ON TABLE conn_output TYPE option<object> READONLY;