use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use flate2::read::DeflateDecoder;

use crate::error::ImportError;

const EOCD_SIG: u32 = 0x0605_4b50;
const CD_SIG: u32 = 0x0201_4b50;
const LFH_SIG: u32 = 0x0403_4b50;
const MAX_EOCD_SEARCH: u64 = 65_535 + 22;
const MAX_CD_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ZipEntryMeta {
    pub name: String,
    pub method: u16,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub local_header_offset: u64,
}

/// Leitor ZIP somente leitura e em fluxo (sem carregar o pacote inteiro em RAM).
pub struct ZipStream {
    file: File,
    entries: HashMap<String, ZipEntryMeta>,
}

impl ZipStream {
    pub fn open(path: &Path) -> Result<Self, ImportError> {
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();
        if file_len < 22 {
            return Err(ImportError::InvalidArchive(
                "arquivo muito pequeno para ser um pacote ZIP/.docx".into(),
            ));
        }

        let search_len = file_len.min(MAX_EOCD_SEARCH) as usize;
        let search_start = file_len - search_len as u64;
        file.seek(SeekFrom::Start(search_start))?;
        let mut tail = vec![0u8; search_len];
        file.read_exact(&mut tail)?;

        let eocd_pos = find_eocd(&tail).ok_or_else(|| {
            ImportError::InvalidArchive("assinatura EOCD do ZIP não encontrada".into())
        })?;
        let eocd = &tail[eocd_pos..];
        let total_entries = read_u16(eocd, 10) as usize;
        let cd_size = read_u32(eocd, 12) as u64;
        let cd_offset = read_u32(eocd, 16) as u64;

        if cd_size as usize > MAX_CD_BYTES || cd_offset.saturating_add(cd_size) > file_len {
            return Err(ImportError::InvalidArchive(
                "diretório central do ZIP inválido ou fora dos limites".into(),
            ));
        }

        file.seek(SeekFrom::Start(cd_offset))?;
        let mut cd = vec![0u8; cd_size as usize];
        file.read_exact(&mut cd)?;

        let mut entries = HashMap::with_capacity(total_entries.min(4096));
        let mut pos = 0usize;
        while pos + 46 <= cd.len() {
            if read_u32(&cd, pos) != CD_SIG {
                return Err(ImportError::InvalidArchive(
                    "entrada do diretório central corrompida".into(),
                ));
            }
            let method = read_u16(&cd, pos + 10);
            let mut compressed_size = read_u32(&cd, pos + 20) as u64;
            let mut uncompressed_size = read_u32(&cd, pos + 24) as u64;
            let name_len = read_u16(&cd, pos + 28) as usize;
            let extra_len = read_u16(&cd, pos + 30) as usize;
            let comment_len = read_u16(&cd, pos + 32) as usize;
            let mut local_header_offset = read_u32(&cd, pos + 42) as u64;

            let name_start = pos + 46;
            let extra_start = name_start + name_len;
            let next_pos = extra_start + extra_len + comment_len;
            if next_pos > cd.len() {
                return Err(ImportError::InvalidArchive(
                    "metadados do diretório central truncados".into(),
                ));
            }
            let raw_name = &cd[name_start..extra_start];
            let name = String::from_utf8_lossy(raw_name).replace('\\', "/");

            if compressed_size == 0xFFFF_FFFF
                || uncompressed_size == 0xFFFF_FFFF
                || local_header_offset == 0xFFFF_FFFF
            {
                let extra = &cd[extra_start..extra_start + extra_len];
                parse_zip64_extra(
                    extra,
                    &mut uncompressed_size,
                    &mut compressed_size,
                    &mut local_header_offset,
                );
            }

            entries.insert(
                name.clone(),
                ZipEntryMeta {
                    name,
                    method,
                    compressed_size,
                    uncompressed_size,
                    local_header_offset,
                },
            );
            pos = next_pos;
        }

        Ok(Self { file, entries })
    }

    pub fn entry_meta(&self, name: &str) -> Option<&ZipEntryMeta> {
        self.entries.get(name)
    }

    pub fn media_entries(&self) -> impl Iterator<Item = &ZipEntryMeta> {
        self.entries
            .values()
            .filter(|e| e.name.starts_with("word/media/") && !e.name.ends_with('/'))
    }

    /// Abre uma parte do ZIP em modo streaming com teto estrito de bytes descomprimidos.
    pub fn open_part(
        &mut self,
        name: &str,
        max_decompressed: u64,
    ) -> Result<LimitedPartReader<'_>, ImportError> {
        let meta = self
            .entries
            .get(name)
            .cloned()
            .ok_or_else(|| ImportError::MissingPart(name.to_string()))?;

        // Valida cabeçalho local antes de ler o payload.
        self.file.seek(SeekFrom::Start(meta.local_header_offset))?;
        let mut lfh = [0u8; 30];
        self.file.read_exact(&mut lfh).map_err(|_| {
            ImportError::InvalidArchive(format!("cabeçalho local truncado em '{name}'"))
        })?;
        if read_u32(&lfh, 0) != LFH_SIG {
            return Err(ImportError::InvalidArchive(format!(
                "assinatura de cabeçalho local inválida em '{name}'"
            )));
        }
        let lfh_name_len = read_u16(&lfh, 26) as u64;
        let lfh_extra_len = read_u16(&lfh, 28) as u64;
        let data_offset = meta
            .local_header_offset
            .checked_add(30)
            .and_then(|v| v.checked_add(lfh_name_len))
            .and_then(|v| v.checked_add(lfh_extra_len))
            .ok_or_else(|| ImportError::InvalidArchive(format!("offset inválido em '{name}'")))?;
        self.file.seek(SeekFrom::Start(data_offset))?;

        let raw_take = (&mut self.file).take(meta.compressed_size);
        let inner = match meta.method {
            0 => PartStream::Stored(raw_take),
            8 => PartStream::Deflate(DeflateDecoder::new(raw_take)),
            other => {
                return Err(ImportError::InvalidArchive(format!(
                    "método de compressão ZIP não suportado ({other}) em '{name}'"
                )));
            }
        };

        Ok(LimitedPartReader {
            part_name: meta.name,
            inner,
            read_so_far: 0,
            limit: max_decompressed,
            bomb_triggered: false,
        })
    }

    /// Lê uma parte pequena (ex. `styles.xml`, `.rels`) com limite rígido e sem alocar além do teto.
    pub fn read_part_to_vec(
        &mut self,
        name: &str,
        max_decompressed: u64,
    ) -> Result<Vec<u8>, ImportError> {
        let mut reader = self.open_part(name, max_decompressed)?;
        let mut out = Vec::new();
        let mut buf = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let cap_limit = max_decompressed as usize;
                    let needed = out.len().saturating_add(n);
                    if needed > out.capacity() {
                        let next = (out.capacity().saturating_mul(2))
                            .max(needed)
                            .min(cap_limit);
                        out.reserve_exact(next.saturating_sub(out.len()));
                    }
                    out.extend_from_slice(&buf[..n]);
                }
                Err(err) => {
                    if reader.bomb_triggered {
                        return Err(ImportError::ZipBomb {
                            part: reader.part_name,
                            limit: max_decompressed,
                        });
                    }
                    return Err(ImportError::InvalidArchive(format!(
                        "falha ao descomprimir '{}': {err}",
                        reader.part_name
                    )));
                }
            }
        }
        Ok(out)
    }

    /// Mede o tamanho descomprimido real de uma parte em streaming (descarta os bytes).
    /// Útil para validar imagens sem carregá-las em RAM.
    pub fn verify_part_size(&mut self, name: &str, limit: u64) -> Result<u64, ImportError> {
        let mut reader = self.open_part(name, limit)?;
        let mut buf = [0u8; 16 * 1024];
        let mut total = 0u64;
        loop {
            match reader.read(&mut buf) {
                Ok(0) => return Ok(total),
                Ok(n) => total = total.saturating_add(n as u64),
                Err(_) if reader.bomb_triggered => {
                    return Err(ImportError::ZipBomb {
                        part: reader.part_name,
                        limit,
                    });
                }
                Err(err) => {
                    return Err(ImportError::InvalidArchive(format!(
                        "falha ao ler '{}': {err}",
                        reader.part_name
                    )));
                }
            }
        }
    }
}

enum PartStream<'a> {
    Stored(io::Take<&'a mut File>),
    Deflate(DeflateDecoder<io::Take<&'a mut File>>),
}

impl Read for PartStream<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Stored(r) => r.read(buf),
            Self::Deflate(r) => r.read(buf),
        }
    }
}

/// Leitor streaming que interrompe imediatamente ao exceder `limit` bytes descomprimidos.
pub struct LimitedPartReader<'a> {
    part_name: String,
    inner: PartStream<'a>,
    read_so_far: u64,
    limit: u64,
    bomb_triggered: bool,
}

impl LimitedPartReader<'_> {
    pub fn bomb_triggered(&self) -> bool {
        self.bomb_triggered
    }

    pub fn part_name(&self) -> &str {
        &self.part_name
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }
}

impl Read for LimitedPartReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.bomb_triggered {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "zip_bomb_limit_exceeded",
            ));
        }
        let remaining = self.limit.saturating_sub(self.read_so_far);
        // Permite ler até remaining + 1 para detectar estouro sem ultrapassar o teto.
        let max_to_read = (remaining.saturating_add(1)).min(buf.len() as u64) as usize;
        if max_to_read == 0 {
            self.bomb_triggered = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "zip_bomb_limit_exceeded",
            ));
        }
        let n = self.inner.read(&mut buf[..max_to_read])?;
        if n == 0 {
            return Ok(0);
        }
        self.read_so_far = self.read_so_far.saturating_add(n as u64);
        if self.read_so_far > self.limit {
            self.bomb_triggered = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "zip_bomb_limit_exceeded",
            ));
        }
        Ok(n)
    }
}

fn find_eocd(tail: &[u8]) -> Option<usize> {
    if tail.len() < 22 {
        return None;
    }
    for i in (0..=tail.len() - 22).rev() {
        if read_u32(tail, i) == EOCD_SIG {
            let comment_len = read_u16(tail, i + 20) as usize;
            if i + 22 + comment_len == tail.len() {
                return Some(i);
            }
        }
    }
    None
}

fn parse_zip64_extra(
    extra: &[u8],
    uncompressed: &mut u64,
    compressed: &mut u64,
    local_offset: &mut u64,
) {
    let mut pos = 0;
    while pos + 4 <= extra.len() {
        let header_id = read_u16(extra, pos);
        let data_size = read_u16(extra, pos + 2) as usize;
        let data_start = pos + 4;
        let data_end = data_start.saturating_add(data_size);
        if data_end > extra.len() {
            break;
        }
        if header_id == 0x0001 {
            let mut off = data_start;
            if *uncompressed == 0xFFFF_FFFF && off + 8 <= data_end {
                *uncompressed = read_u64(extra, off);
                off += 8;
            }
            if *compressed == 0xFFFF_FFFF && off + 8 <= data_end {
                *compressed = read_u64(extra, off);
                off += 8;
            }
            if *local_offset == 0xFFFF_FFFF && off + 8 <= data_end {
                *local_offset = read_u64(extra, off);
            }
            break;
        }
        pos = data_end;
    }
}

fn read_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn read_u64(buf: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        buf[off],
        buf[off + 1],
        buf[off + 2],
        buf[off + 3],
        buf[off + 4],
        buf[off + 5],
        buf[off + 6],
        buf[off + 7],
    ])
}
