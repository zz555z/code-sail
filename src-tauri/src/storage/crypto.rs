use aes::Aes256;
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use std::{fs, path::Path, sync::OnceLock};

use super::SqliteConnection;
use super::SqlValue;

const TOKEN_KEY_FILE_NAME: &str = "codex-config-desktop.key";
const TOKEN_PREFIX: &str = "enc:v1:";

static TOKEN_KEY_CACHE: OnceLock<[u8; 32]> = OnceLock::new();

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

pub(crate) fn encrypt_optional_token(config_path: &Path, token: Option<&str>) -> Result<Option<String>> {
    token
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| encrypt_token(config_path, token))
        .transpose()
}

pub(crate) fn decrypt_optional_token(config_path: &Path, token: Option<&str>) -> Result<Option<String>> {
    token
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| decrypt_token(config_path, token))
        .transpose()
}

fn encrypt_token(config_path: &Path, token: &str) -> Result<String> {
    let key = load_or_create_token_key(config_path)?;
    let mut iv = [0_u8; 16];
    getrandom::fill(&mut iv).map_err(|error| anyhow!("failed to generate token iv: {error}"))?;

    let ciphertext = Aes256CbcEnc::new((&key[..]).into(), (&iv[..]).into())
        .encrypt_padded_vec_mut::<Pkcs7>(token.as_bytes());
    let mut encoded = Vec::with_capacity(iv.len() + ciphertext.len());
    encoded.extend_from_slice(&iv);
    encoded.extend_from_slice(&ciphertext);
    Ok(format!("{TOKEN_PREFIX}{}", BASE64.encode(encoded)))
}

fn decrypt_token(config_path: &Path, token: &str) -> Result<String> {
    if !token.starts_with(TOKEN_PREFIX) {
        return Ok(token.to_string());
    }

    let encoded = token.trim_start_matches(TOKEN_PREFIX);
    let decoded = BASE64.decode(encoded).context("token 密文不是有效 base64")?;
    if decoded.len() <= 16 {
        bail!("token 密文格式不完整");
    }

    let key = load_or_create_token_key(config_path)?;
    let iv = &decoded[..16];
    let ciphertext = &decoded[16..];
    let decrypted = Aes256CbcDec::new((&key[..]).into(), iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .map_err(|_| anyhow!("token 解密失败"))?;
    String::from_utf8(decrypted).context("token 明文不是有效 UTF-8")
}

fn load_or_create_token_key(config_path: &Path) -> Result<[u8; 32]> {
    if let Some(cached) = TOKEN_KEY_CACHE.get() {
        return Ok(*cached);
    }

    let key = load_or_create_token_key_from_disk(config_path)?;
    let _ = TOKEN_KEY_CACHE.set(key);
    Ok(key)
}

fn load_or_create_token_key_from_disk(config_path: &Path) -> Result<[u8; 32]> {
    let key_path = token_key_path(config_path)?;
    if key_path.exists() {
        let raw = fs::read_to_string(&key_path)
            .with_context(|| format!("failed to read {}", key_path.display()))?;
        let decoded = BASE64
            .decode(raw.trim())
            .with_context(|| format!("failed to decode {}", key_path.display()))?;
        if decoded.len() != 32 {
            bail!("token key 文件格式错误: {}", key_path.display());
        }
        let mut key = [0_u8; 32];
        key.copy_from_slice(&decoded);
        return Ok(key);
    }

    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut key = [0_u8; 32];
    getrandom::fill(&mut key).map_err(|error| anyhow!("failed to generate token key: {error}"))?;
    fs::write(&key_path, format!("{}\n", BASE64.encode(key)))
        .with_context(|| format!("failed to write {}", key_path.display()))?;
    super::restrict_file_permissions(&key_path)?;
    Ok(key)
}

fn token_key_path(config_path: &Path) -> Result<std::path::PathBuf> {
    let config_dir = config_path
        .parent()
        .context("failed to locate config directory")?;
    Ok(config_dir.join(TOKEN_KEY_FILE_NAME))
}

pub(crate) fn encrypt_plaintext_tokens(conn: &SqliteConnection, config_path: &Path) -> Result<()> {
    let tokens = conn.query_all(
        r#"
        SELECT id, token
        FROM providers
        WHERE token IS NOT NULL AND length(trim(token)) > 0
        "#,
        &[],
        |row| Ok((row.string(0)?, row.string(1)?)),
    )?;

    for (provider_id, token) in tokens {
        if token.starts_with(TOKEN_PREFIX) {
            continue;
        }
        let encrypted = encrypt_token(config_path, &token)?;
        conn.execute(
            "UPDATE providers SET token = ?1 WHERE id = ?2",
            &[SqlValue::Text(&encrypted), SqlValue::Text(&provider_id)],
        )?;
    }

    Ok(())
}
