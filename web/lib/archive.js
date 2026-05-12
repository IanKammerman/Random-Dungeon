// Fetch and parse archived per-epoch data from web/public/archives/.
// Each epoch contains btc.json, drand.json, nws.json, usgs.json, manifest.json.

const ARCHIVE_ROOT = "public/archives";

export async function listEpochs() {
  // Static hosting can't directory-list; we hardcode the known epoch ids
  // populated by `cargo run -p oracle --bin entropy_once`. To add a new
  // epoch, run the binary and append its number here.
  return [1, 2, 3, 4];
}

export async function loadEpoch(epoch) {
  const base = `${ARCHIVE_ROOT}/${epoch}`;
  const [manifest, btc, drand, usgs, nwsText] = await Promise.all([
    fetchJson(`${base}/manifest.json`),
    fetchJson(`${base}/btc.json`),
    fetchJson(`${base}/drand.json`),
    fetchJson(`${base}/usgs.json`),
    fetchText(`${base}/nws.json`),
  ]);
  const nws = parseNwsNdjson(nwsText);
  return { epoch, manifest, btc, drand, usgs, nws, nwsText };
}

async function fetchJson(url) {
  const r = await fetch(url, { cache: "no-cache" });
  if (!r.ok) throw new Error(`fetch ${url}: ${r.status}`);
  return r.json();
}

async function fetchText(url) {
  const r = await fetch(url, { cache: "no-cache" });
  if (!r.ok) throw new Error(`fetch ${url}: ${r.status}`);
  return r.text();
}

// NWS archive format is newline-delimited JSON: one observation per
// line, each with shape {station: "KJFK", observation: {properties: ...}}.
function parseNwsNdjson(text) {
  const stations = [];
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    stations.push(JSON.parse(line));
  }
  return stations;
}

export async function loadSnarkInputs() {
  return fetchJson("public/snark/public_inputs.json");
}

// Optional sidecar: which epoch's seed was used as alpha for the proof.
// Returns null if not present (older deployments) so the UI can degrade.
export async function loadSnarkMeta() {
  try {
    return await fetchJson("public/snark/snark_meta.json");
  } catch {
    return null;
  }
}

// On-chain deploy info, written by `scripts/deploy-devnet.sh`. May be a
// placeholder with `status: "pending"` until a teammate runs the script
// with a funded wallet. Returns null if the file is missing entirely.
export async function loadDeployInfo() {
  try {
    return await fetchJson("public/deploy.json");
  } catch {
    return null;
  }
}

// ----- per-source headline statistics, computed from real archive data -----

export function btcStat(btcJson) {
  // blockchain.info /latestblock returns {hash, time, height, ...}.
  const time = new Date(btcJson.time * 1000);
  return {
    text: `block #${btcJson.height.toLocaleString()} at ${formatHHMM(time)} UTC`,
    detail: `hash ${btcJson.hash.slice(0, 16)}…`,
  };
}

export function drandStat(drandJson) {
  return {
    text: `round ${drandJson.round.toLocaleString()}`,
    detail: `randomness ${drandJson.randomness.slice(0, 16)}…`,
  };
}

export function usgsStat(usgsJson) {
  const features = usgsJson.features || [];
  const count = features.length;
  let maxMag = null;
  for (const f of features) {
    const m = f.properties && f.properties.mag;
    if (typeof m === "number" && (maxMag === null || m > maxMag)) maxMag = m;
  }
  if (count === 0) return { text: "0 earthquakes in the past hour", detail: "" };
  const magText = maxMag === null ? "no magnitudes reported" : `max magnitude ${maxMag.toFixed(1)}`;
  return {
    text: `${count} earthquakes in the past hour, ${magText}`,
    detail: "",
  };
}

export function nwsStat(nwsArr) {
  const count = nwsArr.length;
  // pick the most recent timestamp across stations
  let latest = null;
  for (const entry of nwsArr) {
    const ts = entry.observation && entry.observation.properties && entry.observation.properties.timestamp;
    if (!ts) continue;
    const t = new Date(ts);
    if (!latest || t > latest) latest = t;
  }
  const hhmm = latest ? formatHHMM(latest) : "—";
  return {
    text: `${count} stations, latest reading at ${hhmm} UTC`,
    detail: "",
  };
}

function formatHHMM(date) {
  const hh = String(date.getUTCHours()).padStart(2, "0");
  const mm = String(date.getUTCMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
}
