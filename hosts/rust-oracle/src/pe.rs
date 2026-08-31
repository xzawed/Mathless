//! Minimal, dependency-free PE symbol-table reader — MEASURES the exported and imported
//! symbol sets of a produced module (SPEC §3-C protection measurement). Handles PE32 and
//! PE32+ (Windows x64 is PE32+, D22); returns an error on layouts it does not understand.

use std::path::Path;

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64le(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}

/// The headers both readers need: the section map (for RVA → file offset) and the data
/// directory. Parsed once so the export and import readers cannot drift apart.
struct Pe<'a> {
    data: &'a [u8],
    /// `(virtual_addr, virtual_size, raw_offset)` per section.
    sections: Vec<(usize, usize, usize)>,
    /// Absolute file offset of `DataDirectory[0]`, and how many entries it has.
    dd_at: usize,
    num_rva: usize,
    /// PE32+ (`0x20B`) uses 8-byte thunks; PE32 uses 4.
    plus: bool,
}

impl<'a> Pe<'a> {
    fn need(&self, off: usize, n: usize) -> Result<(), String> {
        if off + n > self.data.len() {
            Err(format!("truncated PE (need {n} bytes at {off})"))
        } else {
            Ok(())
        }
    }

    fn parse(data: &'a [u8]) -> Result<Pe<'a>, String> {
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

        Ok(Pe {
            data,
            sections,
            dd_at: opt_off + dd_off,
            num_rva,
            plus: magic == 0x20B,
        })
    }

    /// `DataDirectory[index]` as `(rva, size)`; `None` when absent or empty.
    fn dir(&self, index: usize) -> Result<Option<(usize, usize)>, String> {
        if index >= self.num_rva {
            return Ok(None);
        }
        let at = self.dd_at + index * 8;
        self.need(at, 8)?;
        let rva = u32le(self.data, at) as usize;
        let size = u32le(self.data, at + 4) as usize;
        Ok((rva != 0 && size != 0).then_some((rva, size)))
    }

    fn rva_to_off(&self, rva: usize) -> Option<usize> {
        let data_len = self.data.len();
        self.sections.iter().find_map(|&(va, sz, raw)| {
            if rva >= va && rva < va + sz {
                let off = raw + (rva - va);
                // Reject offsets outside the file (malformed PE) so no caller — including
                // the name string reads — can index past EOF.
                (off <= data_len).then_some(off)
            } else {
                None
            }
        })
    }

    /// The NUL-terminated string at `rva`.
    fn cstr(&self, rva: usize) -> Result<String, String> {
        let off = self.rva_to_off(rva).ok_or("name RVA not in any section")?;
        let mut end = off;
        while end < self.data.len() && self.data[end] != 0 {
            end += 1;
        }
        std::str::from_utf8(&self.data[off..end])
            .map(|s| s.to_string())
            .map_err(|_| "non-utf8 symbol name".to_string())
    }
}

/// Read the exported function names of a PE file at `path` (sorted).
pub fn read_exports(path: &Path) -> Result<Vec<String>, String> {
    let data = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    read_exports_bytes(&data)
}

/// Read the exported function names from PE bytes (sorted).
pub fn read_exports_bytes(data: &[u8]) -> Result<Vec<String>, String> {
    let pe = Pe::parse(data)?;
    let Some((export_rva, _)) = pe.dir(0)? else {
        return Ok(vec![]); // no export table
    };

    // IMAGE_EXPORT_DIRECTORY.
    let exp_off = pe
        .rva_to_off(export_rva)
        .ok_or("export RVA not in any section")?;
    pe.need(exp_off, 40)?;
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
    let names_off = pe
        .rva_to_off(address_of_names)
        .ok_or("name table RVA not in any section")?;

    let mut out = Vec::with_capacity(number_of_names);
    for i in 0..number_of_names {
        let p = names_off + i * 4;
        pe.need(p, 4)?;
        out.push(pe.cstr(u32le(data, p) as usize)?);
    }
    out.sort();
    Ok(out)
}

/// Read the imported symbols of a PE file at `path`, as `"DLL!function"` (sorted).
pub fn read_imports(path: &Path) -> Result<Vec<String>, String> {
    let data = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    read_imports_bytes(&data)
}

/// Read the imported symbols from PE bytes, as `"DLL!function"` (sorted, lowercased DLL).
///
/// This is what makes "the import set is unchanged" a MEASUREMENT rather than a claim: a
/// generated module that started calling `strcmp` would gain an `api-ms-win-crt-string!strcmp`
/// entry here, and the CRT heap import is exactly how Q12 was decided.
///
/// Ordinal-only imports are reported as `DLL!#<ordinal>` — the generated modules have none,
/// but silently dropping them would make an added one invisible, which is the failure mode
/// this reader exists to prevent.
pub fn read_imports_bytes(data: &[u8]) -> Result<Vec<String>, String> {
    let pe = Pe::parse(data)?;
    let Some((imp_rva, _)) = pe.dir(1)? else {
        return Ok(vec![]); // no import table
    };
    let mut off = pe
        .rva_to_off(imp_rva)
        .ok_or("import RVA not in any section")?;

    let mut out = Vec::new();
    // IMAGE_IMPORT_DESCRIPTOR is 20 bytes; the array ends at an all-zero entry.
    loop {
        pe.need(off, 20)?;
        let original_first_thunk = u32le(data, off) as usize;
        let name_rva = u32le(data, off + 12) as usize;
        let first_thunk = u32le(data, off + 16) as usize;
        if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
            break;
        }
        let dll = pe.cstr(name_rva)?.to_ascii_lowercase();

        // Prefer the import name table (OriginalFirstThunk); a bound import overwrites the
        // IAT with addresses, and only the INT still names the functions.
        let thunks = if original_first_thunk != 0 {
            original_first_thunk
        } else {
            first_thunk
        };
        let mut t = pe
            .rva_to_off(thunks)
            .ok_or("thunk RVA not in any section")?;
        let (step, ord_flag) = if pe.plus {
            (8usize, 1u64 << 63)
        } else {
            (4usize, 1u64 << 31)
        };
        loop {
            pe.need(t, step)?;
            let entry = if pe.plus {
                u64le(data, t)
            } else {
                u32le(data, t) as u64
            };
            if entry == 0 {
                break;
            }
            if entry & ord_flag != 0 {
                out.push(format!("{dll}!#{}", entry & 0xFFFF));
            } else {
                // IMAGE_IMPORT_BY_NAME: 2-byte hint, then the NUL-terminated name.
                out.push(format!("{dll}!{}", pe.cstr(entry as usize + 2)?));
            }
            t += step;
        }
        off += 20;
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
        assert!(read_imports_bytes(b"not a pe file at all").is_err());
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

    /// The import reader must survive the same malformed inputs the export reader does —
    /// it walks two nested tables, so it has more places to index past EOF, not fewer.
    #[test]
    fn the_import_reader_errors_rather_than_panicking() {
        for b in [&b""[..], &[0u8; 64][..], &[0u8; 96][..]] {
            assert!(read_imports_bytes(b).is_err(), "{b:?}");
        }
    }
}
