//! Minimal, dependency-free PE export-table reader — MEASURES the exported symbol set
//! of a produced module (SPEC §3-C protection measurement). Handles PE32 and PE32+
//! (Windows x64 is PE32+, D22); returns an error on layouts it does not understand.

use std::path::Path;

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Read the exported function names of a PE file at `path` (sorted).
pub fn read_exports(path: &Path) -> Result<Vec<String>, String> {
    let data = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    read_exports_bytes(&data)
}

/// Read the exported function names from PE bytes (sorted).
pub fn read_exports_bytes(data: &[u8]) -> Result<Vec<String>, String> {
    let need = |off: usize, n: usize| -> Result<(), String> {
        if off + n > data.len() {
            Err(format!("truncated PE (need {n} bytes at {off})"))
        } else {
            Ok(())
        }
    };

    // DOS header → PE header offset.
    need(0x40, 0)?;
    if &data[0..2] != b"MZ" {
        return Err("not a PE file (missing MZ)".into());
    }
    let pe_off = u32le(data, 0x3C) as usize;
    need(pe_off, 24)?;
    if &data[pe_off..pe_off + 4] != b"PE\0\0" {
        return Err("missing PE signature".into());
    }

    let num_sections = u16le(data, pe_off + 6) as usize;
    let size_opt = u16le(data, pe_off + 20) as usize;
    let opt_off = pe_off + 24; // 4 (signature) + 20 (COFF header)
    need(opt_off, 2)?; // optional-header magic

    // Data-directory offset within the optional header depends on PE32 vs PE32+.
    let magic = u16le(data, opt_off);
    let dd_off = match magic {
        0x20B => 112, // PE32+
        0x10B => 96,  // PE32
        other => return Err(format!("unknown optional-header magic {other:#x}")),
    };
    need(opt_off + dd_off, 8)?;
    let num_rva = u32le(data, opt_off + dd_off - 4) as usize; // NumberOfRvaAndSizes
    if num_rva == 0 {
        return Ok(vec![]);
    }
    let export_rva = u32le(data, opt_off + dd_off) as usize; // DataDirectory[0].VirtualAddress
    let export_size = u32le(data, opt_off + dd_off + 4) as usize;
    if export_rva == 0 || export_size == 0 {
        return Ok(vec![]); // no export table
    }

    // Section headers, to translate RVAs to file offsets.
    let sect_off = opt_off + size_opt;
    let mut sections: Vec<(usize, usize, usize)> = Vec::with_capacity(num_sections);
    for i in 0..num_sections {
        let s = sect_off + i * 40;
        need(s, 40)?;
        let vsize = u32le(data, s + 8) as usize;
        let vaddr = u32le(data, s + 12) as usize;
        let rawsize = u32le(data, s + 16) as usize;
        let raw = u32le(data, s + 20) as usize;
        sections.push((vaddr, vsize.max(rawsize), raw));
    }
    let data_len = data.len();
    let rva_to_off = |rva: usize| -> Option<usize> {
        sections.iter().find_map(|&(va, sz, raw)| {
            if rva >= va && rva < va + sz {
                let off = raw + (rva - va);
                // Reject offsets outside the file (malformed PE) so no caller — including
                // the export-name string read — can index past EOF.
                (off <= data_len).then_some(off)
            } else {
                None
            }
        })
    };

    // IMAGE_EXPORT_DIRECTORY.
    let exp_off = rva_to_off(export_rva).ok_or("export RVA not in any section")?;
    need(exp_off, 40)?;
    let number_of_names = u32le(data, exp_off + 24) as usize;
    let address_of_names = u32le(data, exp_off + 32) as usize;
    if number_of_names == 0 {
        return Ok(vec![]);
    }
    // Each name needs a 4-byte pointer in the file, so a plausible count cannot exceed
    // the file size; reject absurd values before allocating (malformed-input DoS guard).
    if number_of_names > data.len() {
        return Err(format!("implausible export name count {number_of_names}"));
    }
    let names_off = rva_to_off(address_of_names).ok_or("name table RVA not in any section")?;

    let mut out = Vec::with_capacity(number_of_names);
    for i in 0..number_of_names {
        let p = names_off + i * 4;
        need(p, 4)?;
        let name_off = rva_to_off(u32le(data, p) as usize).ok_or("name RVA not in any section")?;
        let mut end = name_off;
        while end < data.len() && data[end] != 0 {
            end += 1;
        }
        let name = std::str::from_utf8(&data[name_off..end])
            .map_err(|_| "non-utf8 export name".to_string())?;
        out.push(name.to_string());
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_pe() {
        assert!(read_exports_bytes(b"not a pe file at all").is_err());
    }

    #[test]
    fn errors_on_truncated_optional_header_without_panicking() {
        // MZ + e_lfanew=64 + "PE\0\0", but the buffer ends exactly at the optional
        // header, so reading the magic must be an Err, not an index panic.
        let mut b = vec![0u8; 88];
        b[0] = b'M';
        b[1] = b'Z';
        b[0x3C] = 64;
        b[64] = b'P';
        b[65] = b'E';
        assert!(read_exports_bytes(&b).is_err());
    }

    #[test]
    fn rejects_empty_input() {
        // Too short even for the DOS header — the first `need(0x40, 0)` guard.
        assert!(read_exports_bytes(b"").is_err());
    }

    #[test]
    fn rejects_missing_mz_signature() {
        // Long enough for the DOS header but not a PE ("MZ" absent) — hits the MZ guard
        // (the existing 20-byte case is too short and hits the truncation guard instead).
        let e = read_exports_bytes(&[0u8; 64]).unwrap_err();
        assert!(e.contains("MZ"), "{e}");
    }

    #[test]
    fn rejects_pe_offset_past_eof() {
        // MZ present but e_lfanew points past the buffer → truncated PE header, no panic.
        let mut b = vec![0u8; 64];
        b[0] = b'M';
        b[1] = b'Z';
        b[0x3C] = 200;
        assert!(read_exports_bytes(&b).is_err());
    }

    #[test]
    fn rejects_unknown_optional_header_magic() {
        // MZ + "PE\0\0" + a zero optional-header magic → the unknown-magic guard, no panic.
        let mut b = vec![0u8; 96];
        b[0] = b'M';
        b[1] = b'Z';
        b[0x3C] = 64;
        b[64] = b'P';
        b[65] = b'E';
        // Optional-header magic sits at opt_off = 64 + 24 = 88 and is left 0x0000.
        let e = read_exports_bytes(&b).unwrap_err();
        assert!(e.to_lowercase().contains("magic"), "{e}");
    }
}
