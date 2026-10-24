use rhema_core::ModuleError;

/// Assinaturas binárias conhecidas de módulos do theWord
pub const TW_ENCRYPTED_MAGIC: &[u8; 9] = b"TWENCBMOD";
pub const SQLITE_MAGIC: &[u8; 15] = b"SQLite format 3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleSignature {
    /// Módulo em banco de dados SQLite (.twm, .mybible)
    Sqlite,
    /// Bíblia em texto plano (.ont, .ot, .nt)
    TextBible,
}

/// Detecta a assinatura inicial de um arquivo de módulo.
///
/// Se o módulo contiver a assinatura proprietária `TWENCBMOD`, retorna imediatamente
/// o erro tipado `ModuleError::EncryptedModule`, respeitando as diretrizes de direitos
/// autorais e DRM do RhemaStudio.
pub fn detect_signature(bytes: &[u8]) -> Result<ModuleSignature, ModuleError> {
    if bytes.len() >= 9 && &bytes[0..9] == TW_ENCRYPTED_MAGIC {
        return Err(ModuleError::EncryptedModule);
    }
    if bytes.len() >= 15 && &bytes[0..15] == SQLITE_MAGIC {
        return Ok(ModuleSignature::Sqlite);
    }
    Ok(ModuleSignature::TextBible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_encrypted() {
        let sample = b"TWENCBMOD\x01\x02\x03\x04restante_dos_bytes";
        let res = detect_signature(sample);
        assert!(matches!(res, Err(ModuleError::EncryptedModule)));
    }

    #[test]
    fn test_detect_sqlite() {
        let sample = b"SQLite format 3\0\x10\0\x01\x01";
        assert_eq!(detect_signature(sample).unwrap(), ModuleSignature::Sqlite);
    }

    #[test]
    fn test_detect_text_bible() {
        let sample = b"No principio criou Deus os ceus e a terra\r\n";
        assert_eq!(
            detect_signature(sample).unwrap(),
            ModuleSignature::TextBible
        );
    }
}
