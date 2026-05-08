// JS port of the manifest serialization from oracle/src/entropy/manifest.rs.
//
// Per docs/entropy.md:
//
//   manifest := DOMAIN_TAG
//            || "manifest"
//            || epoch_be_u64
//            || fetched_at_ms_be_i64
//            || source_count_be_u32
//            || record[0] || record[1] || ...
//
//   record := source_name_len_be_u16
//          || source_name_utf8
//          || canonical_hash_32
//
// Records are sorted ascending by source name.

export const DOMAIN_TAG_STR = "random-dungeon/entropy/v1";

const enc = new TextEncoder();

export function utf8(str) {
  return enc.encode(str);
}

// Build the manifest bytes from the enriched manifest.json record.
// `manifest` is the parsed manifest.json object — must have:
//   { epoch, manifest_fetched_at_ms, sources: [{name, canonical_hash}, ...] }
export function buildManifestBytes(manifest) {
  const records = [...manifest.sources]
    .map((s) => ({ name: s.name, hash: hexToBytes(s.canonical_hash) }))
    .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));

  // size: tag (25) + "manifest" (8) + epoch u64 (8) + fetched i64 (8) + count u32 (4)
  // + per record: u16 (2) + name bytes + 32
  let size = utf8(DOMAIN_TAG_STR).length + "manifest".length + 8 + 8 + 4;
  for (const r of records) size += 2 + utf8(r.name).length + 32;

  const buf = new Uint8Array(size);
  const dv = new DataView(buf.buffer);
  let o = 0;

  buf.set(utf8(DOMAIN_TAG_STR), o); o += utf8(DOMAIN_TAG_STR).length;
  buf.set(utf8("manifest"), o); o += "manifest".length;

  // u64 big-endian
  writeU64BE(dv, o, BigInt(manifest.epoch)); o += 8;
  // i64 big-endian
  writeI64BE(dv, o, BigInt(manifest.manifest_fetched_at_ms)); o += 8;
  // u32 big-endian
  dv.setUint32(o, records.length, false); o += 4;

  for (const r of records) {
    const nameBytes = utf8(r.name);
    dv.setUint16(o, nameBytes.length, false); o += 2;
    buf.set(nameBytes, o); o += nameBytes.length;
    buf.set(r.hash, o); o += 32;
  }

  return buf;
}

// Build the seed-input bytes: DOMAIN_TAG || "seed" || manifest_hash.
export function buildSeedInputBytes(manifestHashHex) {
  const tag = utf8(DOMAIN_TAG_STR);
  const sep = utf8("seed");
  const hash = hexToBytes(manifestHashHex);
  const out = new Uint8Array(tag.length + sep.length + hash.length);
  out.set(tag, 0);
  out.set(sep, tag.length);
  out.set(hash, tag.length + sep.length);
  return out;
}

export async function sha256(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return new Uint8Array(digest);
}

// ---- hex helpers ----

export function hexToBytes(hex) {
  if (hex.startsWith("0x") || hex.startsWith("0X")) hex = hex.slice(2);
  if (hex.length % 2 !== 0) throw new Error(`bad hex length: ${hex.length}`);
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export function bytesToHex(bytes) {
  let s = "";
  for (let i = 0; i < bytes.length; i++) {
    s += bytes[i].toString(16).padStart(2, "0");
  }
  return s;
}

// DataView has no setBigUint64/setBigInt64 in IE-shaped browsers, but
// modern targets do. Implement explicit helpers anyway for clarity.
function writeU64BE(dv, off, big) {
  dv.setBigUint64(off, big, false);
}

function writeI64BE(dv, off, big) {
  dv.setBigInt64(off, big, false);
}
