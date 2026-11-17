// Strict, bounded ELF64 little-endian reader for the Linux portable recipe.
// Reports what the dynamic loader will need; never executes or loads the file.
export class ElfError extends Error {}
const need = (ok, message) => { if (!ok) throw new ElfError(message); };

const PT_INTERP = 3, PT_DYNAMIC = 2;
const DT_NULL = 0, DT_NEEDED = 1, DT_STRTAB = 5, DT_RPATH = 15, DT_RUNPATH = 29, DT_FLAGS_1 = 0x6ffffffb;
const DT_VERNEED = 0x6ffffffe, DT_VERNEEDNUM = 0x6fffffff;

/// x86_64 ELF64 executable/shared object: interpreter, DT_NEEDED, RPATH/RUNPATH
/// and, per needed library, every required symbol version (e.g. GLIBC_2.34).
export function elfInfo(bytes) {
  const u16 = at => { need(at >= 0 && at + 2 <= bytes.length, 'Truncated ELF'); return bytes.readUInt16LE(at); };
  const u32 = at => { need(at >= 0 && at + 4 <= bytes.length, 'Truncated ELF'); return bytes.readUInt32LE(at); };
  const u64 = at => {
    need(at >= 0 && at + 8 <= bytes.length, 'Truncated ELF');
    const v = bytes.readBigUInt64LE(at);
    need(v <= BigInt(Number.MAX_SAFE_INTEGER), 'ELF value out of range');
    return Number(v);
  };
  need(bytes.length >= 64 && bytes.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46])), 'Not ELF');
  need(bytes[4] === 2 && bytes[5] === 1, 'Expected ELF64 little-endian');
  const type = u16(16), machine = u16(18);
  need(machine === 62, 'Expected x86_64 ELF');
  need(type === 2 || type === 3, 'Expected an executable or shared object');
  const phoff = u64(32), phentsize = u16(54), phnum = u16(56);
  need(phentsize === 56 && phnum > 0 && phnum <= 128, 'Invalid program headers');
  const segments = [];
  for (let i = 0; i < phnum; i++) {
    const at = phoff + i * phentsize;
    segments.push({ type: u32(at), offset: u64(at + 8), vaddr: u64(at + 16), filesz: u64(at + 32) });
  }
  // Virtual address -> file offset through the PT_LOAD segments.
  const loads = [];
  for (let i = 0; i < phnum; i++) {
    const at = phoff + i * phentsize;
    if (u32(at) === 1) loads.push({ offset: u64(at + 8), vaddr: u64(at + 16), filesz: u64(at + 32) });
  }
  const fileOffset = vaddr => {
    const seg = loads.find(s => vaddr >= s.vaddr && vaddr < s.vaddr + s.filesz);
    need(seg, 'Unmapped ELF address');
    return seg.offset + (vaddr - seg.vaddr);
  };
  const cstr = at => {
    const end = bytes.indexOf(0, at);
    need(at < bytes.length && end > at && end - at <= 4096, 'Invalid ELF string');
    return bytes.toString('latin1', at, end);
  };
  const interpSeg = segments.find(s => s.type === PT_INTERP);
  const interpreter = interpSeg ? cstr(interpSeg.offset) : null;
  const dynSeg = segments.find(s => s.type === PT_DYNAMIC);
  need(dynSeg, 'Not dynamically linked');
  const dyn = [];
  for (let at = dynSeg.offset; at + 16 <= dynSeg.offset + dynSeg.filesz; at += 16) {
    const tag = u64(at), val = u64(at + 8);
    if (tag === DT_NULL) break;
    dyn.push([tag, val]);
    need(dyn.length <= 4096, 'Oversized dynamic section');
  }
  const value = tag => dyn.find(([t]) => t === tag)?.[1];
  const strtab = fileOffset(value(DT_STRTAB) ?? need(false, 'Missing DT_STRTAB'));
  const str = off => cstr(strtab + off);
  const needed = dyn.filter(([t]) => t === DT_NEEDED).map(([, v]) => str(v));
  const rpath = dyn.filter(([t]) => t === DT_RPATH || t === DT_RUNPATH).map(([, v]) => str(v));
  for (const name of needed) need(/^[A-Za-z0-9_.+-]+\.so(\.[0-9]+)*$/.test(name), 'Unsafe DT_NEEDED name');
  // Version needs: Verneed { vn_version u16, vn_cnt u16, vn_file u32, vn_aux u32, vn_next u32 }
  //                Vernaux { vna_hash u32, vna_flags u16, vna_other u16, vna_name u32, vna_next u32 }
  const versions = {};
  const verneed = value(DT_VERNEED);
  if (verneed !== undefined) {
    const count = value(DT_VERNEEDNUM) ?? 0;
    need(count <= 256, 'Oversized version needs');
    let at = fileOffset(verneed);
    for (let i = 0; i < count; i++) {
      const cnt = u16(at + 2), file = str(u32(at + 4)), aux = u32(at + 8), next = u32(at + 12);
      need(cnt <= 256, 'Oversized version aux');
      let a = at + aux;
      const list = versions[file] ?? (versions[file] = []);
      for (let j = 0; j < cnt; j++) {
        list.push(str(u32(a + 8)));
        const n = u32(a + 12);
        if (!n) break;
        a += n;
      }
      if (!next) break;
      at += next;
    }
  }
  for (const list of Object.values(versions)) list.sort(compareVersionTags);
  return { machine: 'x86_64', type: type === 2 ? 'EXEC' : 'DYN', interpreter, needed, rpath,
    flags_1: value(DT_FLAGS_1) ?? 0, versions };
}

/// Orders tags like GLIBC_2.17 < GLIBC_2.34 numerically within one family.
export function compareVersionTags(a, b) {
  const split = s => { const m = /^([A-Z_]+?)_?([0-9][0-9.]*)$/.exec(s); return m ? [m[1], m[2].split('.').map(Number)] : [s, []]; };
  const [fa, va] = split(a), [fb, vb] = split(b);
  if (fa !== fb) return fa < fb ? -1 : 1;
  for (let i = 0; i < Math.max(va.length, vb.length); i++) {
    const d = (va[i] ?? 0) - (vb[i] ?? 0);
    if (d) return d;
  }
  return 0;
}

/// Highest required version per family across all binaries (GLIBC, GLIBCXX, CXXABI, GCC).
export function minimumVersions(infos) {
  const best = {};
  for (const info of infos) {
    for (const list of Object.values(info.versions)) {
      for (const tag of list) {
        const family = /^([A-Z_]+?)_?[0-9]/.exec(tag)?.[1];
        if (!family) continue;
        if (!best[family] || compareVersionTags(best[family], tag) < 0) best[family] = tag;
      }
    }
  }
  return best;
}
