use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ServerError;

// --- Permission Levels ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "permission_level", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    Admin,
    Moderator,
    User,
}

impl std::fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::Moderator => write!(f, "moderator"),
            Self::User => write!(f, "user"),
        }
    }
}

// --- User Model ---

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub display_name: String,
    pub permission_level: PermissionLevel,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

// --- JWT Claims ---

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user ID
    pub username: String,
    pub display_name: String,
    pub permission: PermissionLevel,
    pub exp: i64,
    pub iat: i64,
}

/// Authenticated user info extracted from JWT.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub permission: PermissionLevel,
}

// --- Password Hashing ---

pub fn hash_password(password: &str) -> Result<String, ServerError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ServerError::InternalError(format!("password hash error: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, ServerError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| ServerError::InternalError(format!("invalid password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

// --- JWT Token Operations ---

const ACCESS_TOKEN_EXPIRY_MINUTES: i64 = 15;
const REFRESH_TOKEN_EXPIRY_DAYS: i64 = 7;

pub fn create_access_token(user: &User, secret: &str) -> Result<String, ServerError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user.id.to_string(),
        username: user.username.clone(),
        display_name: user.display_name.clone(),
        permission: user.permission_level,
        iat: now.timestamp(),
        exp: (now + Duration::minutes(ACCESS_TOKEN_EXPIRY_MINUTES)).timestamp(),
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ServerError::InternalError(format!("JWT encode error: {e}")))
}

pub fn validate_access_token(token: &str, secret: &str) -> Result<AuthUser, ServerError> {
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| ServerError::Unauthorized)?;

    let user_id = Uuid::parse_str(&data.claims.sub).map_err(|_| ServerError::Unauthorized)?;

    Ok(AuthUser {
        id: user_id,
        username: data.claims.username,
        display_name: data.claims.display_name,
        permission: data.claims.permission,
    })
}

// --- Refresh Token Operations ---

pub fn generate_refresh_token() -> String {
    Uuid::new_v4().to_string()
}

pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn refresh_token_expiry() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRY_DAYS)
}

// --- Database Operations ---

pub async fn find_user_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<User>, ServerError> {
    let row: Option<User> = sqlx::query_as(
        r#"SELECT id, username, password_hash, display_name,
                  permission_level, created_at, updated_at
           FROM users WHERE username = $1"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(row)
}

pub async fn find_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, ServerError> {
    let row: Option<User> = sqlx::query_as(
        r#"SELECT id, username, password_hash, display_name,
                  permission_level, created_at, updated_at
           FROM users WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(row)
}

pub async fn create_user(
    pool: &PgPool,
    username: &str,
    password: &str,
    display_name: &str,
    permission: PermissionLevel,
) -> Result<User, ServerError> {
    let password_hash = hash_password(password)?;
    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, display_name, permission_level, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(username)
    .bind(&password_hash)
    .bind(display_name)
    .bind(permission)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e
            && db_err.constraint() == Some("users_username_key")
        {
            return ServerError::InvalidAction("Username already exists".to_string());
        }
        ServerError::InternalError(format!("database error: {e}"))
    })?;

    Ok(User {
        id,
        username: username.to_string(),
        password_hash,
        display_name: display_name.to_string(),
        permission_level: permission,
        created_at: now,
        updated_at: now,
    })
}

pub async fn store_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: chrono::DateTime<Utc>,
) -> Result<(), ServerError> {
    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(pool)
        .await
        .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(())
}

pub async fn validate_refresh_token(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<Uuid>, ServerError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM refresh_tokens
         WHERE token_hash = $1 AND revoked = FALSE AND expires_at > NOW()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(row.map(|(uid,)| uid))
}

pub async fn revoke_refresh_token(pool: &PgPool, token_hash: &str) -> Result<(), ServerError> {
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await
        .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(())
}

pub async fn revoke_all_user_tokens(pool: &PgPool, user_id: Uuid) -> Result<(), ServerError> {
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(())
}

pub async fn list_all_users(pool: &PgPool) -> Result<Vec<User>, ServerError> {
    let rows: Vec<User> = sqlx::query_as(
        r#"SELECT id, username, password_hash, display_name,
                  permission_level, created_at, updated_at
           FROM users ORDER BY created_at ASC"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(rows)
}

pub async fn update_user_permission(
    pool: &PgPool,
    user_id: Uuid,
    permission: PermissionLevel,
) -> Result<(), ServerError> {
    let result =
        sqlx::query("UPDATE users SET permission_level = $1, updated_at = NOW() WHERE id = $2")
            .bind(permission)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    if result.rows_affected() == 0 {
        return Err(ServerError::RoomNotFound); // reusing — user not found
    }
    Ok(())
}

pub async fn delete_user(pool: &PgPool, user_id: Uuid) -> Result<(), ServerError> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    if result.rows_affected() == 0 {
        return Err(ServerError::RoomNotFound); // reusing — user not found
    }
    Ok(())
}

pub async fn update_user_password(
    pool: &PgPool,
    user_id: Uuid,
    new_password_hash: &str,
) -> Result<(), ServerError> {
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(new_password_hash)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(())
}

pub async fn update_user_display_name(
    pool: &PgPool,
    user_id: Uuid,
    display_name: &str,
) -> Result<(), ServerError> {
    sqlx::query("UPDATE users SET display_name = $1, updated_at = NOW() WHERE id = $2")
        .bind(display_name)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(())
}

pub async fn get_app_setting(pool: &PgPool, key: &str) -> Result<Option<String>, ServerError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(row.map(|(v,)| v))
}

pub async fn set_app_setting(pool: &PgPool, key: &str, value: &str) -> Result<(), ServerError> {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(())
}

pub async fn user_count(pool: &PgPool) -> Result<i64, ServerError> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| ServerError::InternalError(format!("database error: {e}")))?;

    Ok(count)
}

/// Seed the admin account if no users exist.
pub async fn seed_admin_account(
    pool: &PgPool,
    username: &str,
    password: &str,
) -> Result<bool, ServerError> {
    let count = user_count(pool).await?;
    if count > 0 {
        return Ok(false);
    }

    create_user(pool, username, password, username, PermissionLevel::Admin).await?;
    Ok(true)
}

/// Generate a random password suitable for admin-created accounts.
pub fn generate_random_password() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let chars: Vec<char> = (0..16)
        .map(|_| {
            let idx = rng.random_range(0..62);
            match idx {
                0..=9 => (b'0' + idx) as char,
                10..=35 => (b'a' + idx - 10) as char,
                _ => (b'A' + idx - 36) as char,
            }
        })
        .collect();
    chars.into_iter().collect()
}
