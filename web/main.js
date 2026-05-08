// Entry point for the Random Dungeon visualizer.

import {
  listEpochs,
  loadEpoch,
  loadSnarkInputs,
  btcStat,
  drandStat,
  usgsStat,
  nwsStat,
} from "./lib/archive.js";
import {
  buildManifestBytes,
  buildSeedInputBytes,
  sha256,
  hexToBytes,
  bytesToHex,
  DOMAIN_TAG_STR,
} from "./lib/canonicalize.js";
import {
  hashChip,
  highlightedJson,
  highlightJson,
  el,
  formatUtc,
} from "./lib/render.js";

const SOURCES = [
  {
    name: "btc",
    label: "Bitcoin block hash",
    monogram: "₿",
    endpoint: "https://blockchain.info/latestblock",
    why:
      "A Bitcoin block hash is the output of global proof-of-work and contains 256 bits of cryptographic entropy. Manipulating it costs more than any conceivable attack on the beacon. We accept the latest block; the spec requires one confirmation depth before the seed is consumed.",
  },
  {
    name: "drand",
    label: "drand beacon",
    monogram: "d",
    endpoint: "https://api.drand.sh/public/latest",
    why:
      "drand is a threshold-BLS distributed randomness beacon run by ~18 organizations across four continents. Its output is unpredictable, unbiasable, and cryptographically verifiable against a public group key. Cloudflare's contribution incorporates LavaRand entropy; we treat drand as a single source rather than double-counting.",
  },
  {
    name: "nws",
    label: "NOAA weather",
    monogram: "N",
    endpoint: "https://api.weather.gov/stations/{station}/observations/latest",
    why:
      "Weather is chaotic. The bottom digits of reported instrument readings — temperature, pressure, humidity, wind — are at the noise floor and not predictable by any external party. Five fixed stations across the US (KDEN, KJFK, KLAX, KORD, KSEA) decorrelate regional weather effects.",
  },
  {
    name: "usgs",
    label: "USGS earthquakes",
    monogram: "U",
    endpoint:
      "https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_hour.geojson",
    why:
      "The exact set of earthquake events in the past hour, their magnitudes to two decimals, and their coordinates to milli-degree resolution come from global seismic activity and instrument noise. No single actor can produce, suppress, or predict micro-quakes; the bottom digits of the magnitude readings are at the instrument noise floor.",
  },
];

const STAGE_DEFS = [
  { id: "sources", title: "4 sources", sublabel: "raw bytes" },
  { id: "canonical", title: "canonical bytes", sublabel: "fixed-layout" },
  { id: "manifest", title: "manifest", sublabel: "SHA-256" },
  { id: "seed", title: "seed", sublabel: "32 bytes" },
  { id: "vrf", title: "VRF", sublabel: "+ Groth16" },
];

const state = {
  epochs: [],
  current: null, // loaded epoch object
  snark: null,
  selectedStage: "sources",
};

async function main() {
  state.epochs = await listEpochs();
  state.snark = await loadSnarkInputs();

  const select = document.getElementById("epoch-select");
  for (const e of state.epochs) {
    select.appendChild(el("option", { value: String(e), text: `Epoch ${e}` }));
  }
  // default to most recent
  const defaultEpoch = state.epochs[state.epochs.length - 1];
  select.value = String(defaultEpoch);
  select.addEventListener("change", async () => {
    await selectEpoch(Number(select.value));
  });

  buildPipelineSvg();
  renderSnark(state.snark);

  await selectEpoch(defaultEpoch);
}

async function selectEpoch(epoch) {
  state.current = await loadEpoch(epoch);
  document.getElementById("epoch-meta").textContent =
    `manifest fetched ${formatUtc(state.current.manifest.manifest_fetched_at_ms)}`;
  renderSourceCards(state.current);
  renderPipelinePanel(state.selectedStage);
  renderVerifyCard(state.current);
}

// ---------- source cards ----------

function renderSourceCards(epoch) {
  const grid = document.getElementById("source-grid");
  grid.replaceChildren();

  for (const def of SOURCES) {
    const stat = computeStat(def.name, epoch);
    const card = el("div", { class: "source-card" });

    // header
    card.appendChild(
      el("div", { class: "source-header" }, [
        el("span", { class: "monogram", text: def.monogram }),
        el("div", {}, [
          el("div", { class: "source-name", text: def.name }),
          el("div", { style: "font-size: 14px; color: var(--text)", text: def.label }),
        ]),
      ]),
    );

    // headline stat
    const statEl = el("p", { class: "source-stat" });
    statEl.innerHTML = stat.html;
    card.appendChild(statEl);

    // why
    card.appendChild(el("p", { class: "source-why", text: def.why }));

    // endpoint link
    card.appendChild(
      el("div", { class: "source-endpoint" }, [
        el("a", {
          href: def.endpoint.replace("{station}", "KJFK"),
          target: "_blank",
          rel: "noopener",
          text: def.endpoint,
        }),
      ]),
    );

    // raw response details
    const details = el("details");
    details.appendChild(el("summary", { class: "source-raw-toggle", text: "View raw response" }));
    const pre = document.createElement("pre");
    pre.className = "source-raw";
    pre.innerHTML = highlightedRaw(def.name, epoch);
    details.appendChild(pre);
    card.appendChild(details);

    grid.appendChild(card);
  }
}

function computeStat(name, epoch) {
  if (name === "btc") {
    const s = btcStat(epoch.btc);
    return { html: `<strong>${escapeHtml(s.text)}</strong>` };
  }
  if (name === "drand") {
    const s = drandStat(epoch.drand);
    return { html: `<strong>${escapeHtml(s.text)}</strong>` };
  }
  if (name === "usgs") {
    const s = usgsStat(epoch.usgs);
    return { html: `<strong>${escapeHtml(s.text)}</strong>` };
  }
  if (name === "nws") {
    const s = nwsStat(epoch.nws);
    return { html: `<strong>${escapeHtml(s.text)}</strong>` };
  }
  return { html: "" };
}

function highlightedRaw(name, epoch) {
  if (name === "nws") {
    // NDJSON: highlight each line.
    return epoch.nwsText
      .split("\n")
      .filter((l) => l.trim().length > 0)
      .map((line) => {
        try {
          return highlightedJson(JSON.parse(line));
        } catch {
          return highlightJson(line);
        }
      })
      .join("\n");
  }
  const obj = epoch[name];
  return highlightedJson(obj);
}

function escapeHtml(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// ---------- pipeline ----------

function buildPipelineSvg() {
  const svg = document.getElementById("pipeline-svg");
  svg.replaceChildren();

  // viewBox is 1200x400. Five stages, equal horizontal spacing.
  const W = 1200, H = 400;
  const padX = 40, padY = 80;
  const innerW = W - padX * 2;
  const stages = STAGE_DEFS.length;
  const gap = 40;
  const stageW = (innerW - gap * (stages - 1)) / stages;
  const stageH = 140;
  const stageY = (H - stageH) / 2;

  // arrow marker defs
  const defs = svgEl("defs");
  defs.innerHTML = `
    <marker id="pipeline-arrowhead" viewBox="0 0 10 10" refX="9" refY="5"
            markerUnits="strokeWidth" markerWidth="8" markerHeight="8" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#2a2c3d"/>
    </marker>`;
  svg.appendChild(defs);

  // arrows
  for (let i = 0; i < stages - 1; i++) {
    const x1 = padX + (stageW + gap) * i + stageW;
    const y = stageY + stageH / 2;
    const x2 = padX + (stageW + gap) * (i + 1);
    const arrow = svgEl("line", {
      x1: String(x1 + 4),
      y1: String(y),
      x2: String(x2 - 4),
      y2: String(y),
      class: "pipeline-arrow",
    });
    svg.appendChild(arrow);
  }

  // stages
  STAGE_DEFS.forEach((stage, i) => {
    const g = svgEl("g", { class: "pipeline-stage", "data-stage": stage.id });
    const x = padX + (stageW + gap) * i;
    const rect = svgEl("rect", {
      x: String(x),
      y: String(stageY),
      width: String(stageW),
      height: String(stageH),
      rx: "8",
      ry: "8",
    });
    const label = svgEl("text", {
      class: "label",
      x: String(x + stageW / 2),
      y: String(stageY + stageH / 2 - 4),
    });
    label.textContent = stage.title;
    const sub = svgEl("text", {
      class: "sublabel",
      x: String(x + stageW / 2),
      y: String(stageY + stageH / 2 + 18),
    });
    sub.textContent = stage.sublabel;
    g.appendChild(rect);
    g.appendChild(label);
    g.appendChild(sub);
    g.addEventListener("click", () => {
      state.selectedStage = stage.id;
      renderPipelinePanel(stage.id);
      updatePipelineActive();
    });
    svg.appendChild(g);
  });

  updatePipelineActive();
}

function updatePipelineActive() {
  const svg = document.getElementById("pipeline-svg");
  for (const g of svg.querySelectorAll(".pipeline-stage")) {
    g.classList.toggle("active", g.dataset.stage === state.selectedStage);
  }
}

function svgEl(tag, attrs = {}) {
  const e = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.setAttribute("class", v);
    else e.setAttribute(k, v);
  }
  return e;
}

function renderPipelinePanel(stageId) {
  const panel = document.getElementById("pipeline-panel");
  panel.replaceChildren();
  if (!state.current) return;

  switch (stageId) {
    case "sources":
      panel.appendChild(stageSources(state.current));
      break;
    case "canonical":
      panel.appendChild(stageCanonical(state.current));
      break;
    case "manifest":
      panel.appendChild(stageManifest(state.current));
      break;
    case "seed":
      panel.appendChild(stageSeed(state.current));
      break;
    case "vrf":
      panel.appendChild(stageVrf(state.current, state.snark));
      break;
  }
}

function stageSources(epoch) {
  const root = document.createDocumentFragment();
  root.appendChild(el("h3", { text: "Stage 1 — four sources, four canonical hashes" }));
  root.appendChild(
    el("p", {
      text:
        "Each raw API response is canonicalized to a fixed-layout byte string per docs/entropy.md. The SHA-256 of those bytes is what binds into the manifest.",
    }),
  );
  const grid = el("div", { class: "kv-grid" });
  for (const s of [...epoch.manifest.sources].sort((a, b) => (a.name < b.name ? -1 : 1))) {
    grid.appendChild(el("span", { class: "k", text: `SHA256(${s.name}_canonical)` }));
    const v = el("span", { class: "v" });
    v.appendChild(hashChip(s.canonical_hash, { title: "click to copy" }));
    grid.appendChild(v);
  }
  root.appendChild(grid);
  return root;
}

function stageCanonical(epoch) {
  const root = document.createDocumentFragment();
  root.appendChild(el("h3", { text: "Stage 2 — canonical bytes" }));
  root.appendChild(
    el("p", {
      text:
        "Every canonical record begins with a 25-byte ASCII domain tag (highlighted) followed by the source name. Big-endian integers, fixed-point floats, sorted records — JSON whitespace is never hashed.",
    }),
  );
  const grid = el("div", { class: "kv-grid" });
  grid.appendChild(el("span", { class: "k", text: "DOMAIN_TAG" }));
  grid.appendChild(
    el("span", { class: "v" }, [
      el("span", { class: "byte-tag", text: `b"${DOMAIN_TAG_STR}"` }),
      el("span", { text: "(25 bytes)" }),
    ]),
  );
  for (const s of [...epoch.manifest.sources].sort((a, b) => (a.name < b.name ? -1 : 1))) {
    grid.appendChild(el("span", { class: "k", text: `${s.name}_canonical` }));
    grid.appendChild(
      el("span", { class: "v" }, [
        el("span", { class: "byte-tag", text: `${s.canonical_len} bytes` }),
        el("span", { text: ` starts with `}),
        el("span", { class: "byte-tag", text: `b"${DOMAIN_TAG_STR}${s.name}"` }),
      ]),
    );
  }
  root.appendChild(grid);
  return root;
}

function stageManifest(epoch) {
  const root = document.createDocumentFragment();
  root.appendChild(el("h3", { text: "Stage 3 — manifest" }));
  root.appendChild(
    el("p", {
      text:
        'manifest := DOMAIN_TAG || "manifest" || epoch_be_u64 || fetched_at_ms_be_i64 || count_be_u32 || record[0] || record[1] || ... — records sorted by source name. SHA-256 of those bytes is what goes on-chain.',
    }),
  );
  const grid = el("div", { class: "kv-grid" });
  grid.appendChild(el("span", { class: "k", text: "epoch" }));
  grid.appendChild(el("span", { class: "v", text: String(epoch.manifest.epoch) }));
  grid.appendChild(el("span", { class: "k", text: "fetched_at_ms" }));
  grid.appendChild(
    el("span", { class: "v", text: `${epoch.manifest.manifest_fetched_at_ms} (${formatUtc(epoch.manifest.manifest_fetched_at_ms)})` }),
  );
  grid.appendChild(el("span", { class: "k", text: "source_count" }));
  grid.appendChild(el("span", { class: "v", text: String(epoch.manifest.sources.length) }));
  grid.appendChild(el("span", { class: "k", text: "manifest_hash" }));
  const v = el("span", { class: "v" });
  v.appendChild(hashChip(epoch.manifest.manifest_hash));
  grid.appendChild(v);
  root.appendChild(grid);
  return root;
}

function stageSeed(epoch) {
  const root = document.createDocumentFragment();
  root.appendChild(el("h3", { text: "Stage 4 — seed" }));
  root.appendChild(
    el("p", {
      html:
        's_t := <code>SHA256(b"random-dungeon/entropy/v1" || "seed" || manifest_hash)</code>. One SHA-256 call from the on-chain manifest hash to the VRF input. The "Verify the seed yourself" panel below recomputes this in your browser.',
    }),
  );
  const grid = el("div", { class: "kv-grid" });
  grid.appendChild(el("span", { class: "k", text: "manifest_hash" }));
  const m = el("span", { class: "v" });
  m.appendChild(hashChip(epoch.manifest.manifest_hash));
  grid.appendChild(m);
  grid.appendChild(el("span", { class: "k", text: "seed (s_t)" }));
  const s = el("span", { class: "v" });
  s.appendChild(hashChip(epoch.manifest.seed));
  grid.appendChild(s);
  root.appendChild(grid);
  return root;
}

function stageVrf(epoch, snark) {
  const root = document.createDocumentFragment();
  root.appendChild(el("h3", { text: "Stage 5 — VRF + Groth16" }));
  root.appendChild(
    el("p", {
      html:
        'The seed becomes alpha. The oracle computes <code>alpha_hash = SHA256(alpha) → Fr</code>, <code>h = Poseidon(alpha_hash)</code>, <code>gamma = sk·h</code>, <code>beta = Poseidon(gamma)</code>, and produces a Groth16 proof binding the public inputs <code>[alpha_hash, beta]</code>. The proof is 256 bytes (-A || B || C in BN254 byte format) and verifies on Solana.',
    }),
  );
  const grid = el("div", { class: "kv-grid" });
  grid.appendChild(el("span", { class: "k", text: "alpha_hash" }));
  const a = el("span", { class: "v" });
  a.appendChild(hashChip(snark.alpha_hash));
  grid.appendChild(a);
  grid.appendChild(el("span", { class: "k", text: "beta" }));
  const b = el("span", { class: "v" });
  b.appendChild(hashChip(snark.beta));
  grid.appendChild(b);
  grid.appendChild(el("span", { class: "k", text: "proof" }));
  grid.appendChild(el("span", { class: "v" }, [
    el("span", { class: "byte-tag", text: "256 bytes" }),
    el("span", { text: " — see ", style: "color: var(--text-dim)" }),
    el("a", {
      href: "https://github.com/IanKammerman/Random-Dungeon/blob/main/docs/snark-vrf-integration.md",
      target: "_blank", rel: "noopener", text: "snark-vrf-integration.md",
    }),
  ]));
  root.appendChild(grid);
  return root;
}

// ---------- verify card ----------

function renderVerifyCard(epoch) {
  document.getElementById("verify-manifest-hash").replaceChildren(
    hashChip(epoch.manifest.manifest_hash),
  );
  document.getElementById("verify-archived-seed").replaceChildren(
    hashChip(epoch.manifest.seed),
  );

  const seedBtn = document.getElementById("verify-seed-btn");
  const seedResult = document.getElementById("verify-seed-result");
  seedResult.replaceChildren();
  seedResult.className = "verify-result";
  seedBtn.onclick = async () => {
    seedResult.replaceChildren(el("span", { text: "computing…" }));
    const input = buildSeedInputBytes(epoch.manifest.manifest_hash);
    const out = await sha256(input);
    const hex = bytesToHex(out);
    const archived = (epoch.manifest.seed || "").replace(/^0x/, "");
    const ok = hex === archived;
    seedResult.className = ok ? "verify-result ok" : "verify-result bad";
    seedResult.replaceChildren(
      el("span", { class: "check", text: ok ? "✓" : "✗" }),
      el("span", {
        text: ok
          ? "Match — you just verified this in your browser."
          : "Mismatch — archive may be inconsistent.",
      }),
      el("span", { class: "computed", text: `computed: ${hex}` }),
    );
  };

  const manBtn = document.getElementById("verify-manifest-btn");
  const manResult = document.getElementById("verify-manifest-result");
  manResult.replaceChildren();
  manResult.className = "verify-result";
  manBtn.onclick = async () => {
    manResult.replaceChildren(el("span", { text: "rebuilding manifest…" }));
    const bytes = buildManifestBytes(epoch.manifest);
    const out = await sha256(bytes);
    const hex = bytesToHex(out);
    const archived = (epoch.manifest.manifest_hash || "").replace(/^0x/, "");
    const ok = hex === archived;
    manResult.className = ok ? "verify-result ok" : "verify-result bad";
    manResult.replaceChildren(
      el("span", { class: "check", text: ok ? "✓" : "✗" }),
      el("span", {
        text: ok
          ? `Match — re-derived from ${epoch.manifest.sources.length} canonical hashes + epoch + fetched_at.`
          : "Mismatch — re-derivation does not agree with archived manifest_hash.",
      }),
      el("span", { class: "computed", text: `computed: ${hex}` }),
      el("span", { class: "computed", text: `manifest input: ${bytes.length} bytes` }),
    );
  };
}

// ---------- snark card ----------

function renderSnark(snark) {
  const card = document.getElementById("snark-card");
  card.replaceChildren();

  const grid = el("div", { class: "kv-grid" });
  grid.appendChild(el("span", { class: "k", text: "alpha_hash" }));
  const a = el("span", { class: "v" });
  a.appendChild(hashChip(snark.alpha_hash));
  grid.appendChild(a);
  grid.appendChild(el("span", { class: "k", text: "beta" }));
  const b = el("span", { class: "v" });
  b.appendChild(hashChip(snark.beta));
  grid.appendChild(b);
  card.appendChild(grid);

  card.appendChild(
    el("p", {
      class: "snark-foot",
      html:
        'Proof format: 256 bytes total, encoded as <code>-A || B || C</code> in BN254 byte format (A and C are 64-byte G1 points, B is a 128-byte G2 point). Public inputs: <code>[alpha_hash, beta]</code>.',
    }),
  );
  card.appendChild(
    el("p", { class: "snark-foot" }, [
      el("a", {
        href: "https://github.com/IanKammerman/Random-Dungeon/blob/main/docs/snark-vrf-integration.md",
        target: "_blank", rel: "noopener", text: "docs/snark-vrf-integration.md",
      }),
      el("a", {
        href: "https://github.com/IanKammerman/Random-Dungeon/blob/main/crates/solana-program/src/verifier.rs",
        target: "_blank", rel: "noopener", text: "crates/solana-program/src/verifier.rs",
      }),
    ]),
  );
}

main().catch((err) => {
  console.error(err);
  document.body.innerHTML = `<main><pre style="color:#fda4af;font-family:ui-monospace,monospace;padding:32px">${(err && err.stack) || err}</pre></main>`;
});
