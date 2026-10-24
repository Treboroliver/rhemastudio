use std::path::Path;

use rhema_core::ModuleError;
use rusqlite::{Connection, OpenFlags};

/// Rejeita módulos SQLite cuja configuração declara proteção/encriptação.
///
/// O banco é aberto exclusivamente para leitura e nenhum valor de `content` é consultado.
pub fn ensure_twm_unprotected(path: impl AsRef<Path>) -> Result<(), ModuleError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(ModuleError::NotFound(path.display().to_string()));
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| ModuleError::Corrupted(error.to_string()))?;

    let config_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'config')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| ModuleError::Corrupted(error.to_string()))?;
    if !config_exists {
        return Ok(());
    }

    let mut statement = connection
        .prepare("SELECT name, value FROM config")
        .map_err(|error| ModuleError::Corrupted(error.to_string()))?;
    let mut rows = statement
        .query([])
        .map_err(|error| ModuleError::Corrupted(error.to_string()))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| ModuleError::Corrupted(error.to_string()))?
    {
        let name = row
            .get::<_, Option<String>>(0)
            .map_err(|error| ModuleError::Corrupted(error.to_string()))?;
        let value = row
            .get::<_, Option<String>>(1)
            .map_err(|error| ModuleError::Corrupted(error.to_string()))?;
        let protected_key = name.as_deref().is_some_and(|name| {
            matches!(
                name.trim().to_ascii_lowercase().as_str(),
                "do.encrypt" | "secure"
            )
        });
        let enabled = value.as_deref().is_some_and(|value| {
            matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true")
        });
        if protected_key && enabled {
            return Err(ModuleError::EncryptedModule);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    fn config_module(rows: &[(&str, &str)]) -> NamedTempFile {
        let file = NamedTempFile::new().expect("criar banco sintético");
        let connection = Connection::open(file.path()).expect("abrir banco sintético");
        connection
            .execute_batch("CREATE TABLE config(name TEXT, value TEXT);")
            .expect("criar tabela config sintética");
        for (name, value) in rows {
            connection
                .execute(
                    "INSERT INTO config(name, value) VALUES (?1, ?2)",
                    [name, value],
                )
                .expect("inserir configuração sintética");
        }
        drop(connection);
        file
    }

    #[test]
    fn rejects_declared_encrypted_or_secure_modules() {
        for config in [
            &[("do.encrypt", "1")][..],
            &[("secure", "true")][..],
            &[("DO.ENCRYPT", " 1 ")][..],
        ] {
            let file = config_module(config);
            assert!(matches!(
                ensure_twm_unprotected(file.path()),
                Err(ModuleError::EncryptedModule)
            ));
        }
    }

    #[test]
    fn allows_unprotected_or_missing_config_metadata() {
        let file = config_module(&[("compressed", "1"), ("secure", "0")]);
        assert!(ensure_twm_unprotected(file.path()).is_ok());

        let empty_database = NamedTempFile::new().expect("criar banco sem config");
        Connection::open(empty_database.path())
            .expect("abrir banco sem config")
            .close()
            .expect("fechar banco sem config");
        assert!(ensure_twm_unprotected(empty_database.path()).is_ok());
    }
}
