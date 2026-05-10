# Deploying `randomness_beacon` to Solana devnet — step-by-step

> ✅ **Status:** the program is **already deployed on devnet** at
> [`2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH`](https://explorer.solana.com/address/2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH?cluster=devnet)
> (deployed 2026-05-10). The visualizer's hero badge already links to it.
> This guide remains as the runbook for **re-deploys, upgrades, or fresh
> deploys from a different keypair** — re-running it pushes a buffer-based
> upgrade in place and rewrites `web/public/deploy.json` with the new tx.

**Audience:** the teammate who's actually going to run the deploy.
**Time:** 15–25 minutes including faucet waits.
**Cost:** $0. Devnet SOL has no monetary value and the faucets are free.
**Prerequisite knowledge:** you can open a terminal. That's it.

---

## 0. Before you start

You need:

- A clone of the repo on `main` (or a feature branch off `main`).
- `solana` CLI installed (`solana --version` should print something).
- `anchor` CLI installed (`anchor --version` should print something). The
  build expects whatever version `Anchor.toml` pins — don't upgrade it.
- A **GitHub account** with some history (not a brand-new throwaway). The
  faucet validates GitHub accounts and brand-new ones sometimes get rejected.
- ~25 minutes of uninterrupted time.

If you're missing the CLIs, install them from the official Solana and Anchor
docs first. Don't try to do this without them.

---

## 1. Get on the right branch

```bash
git checkout main
git pull
```

Confirm you see `scripts/deploy-devnet.sh`:

```bash
ls scripts/deploy-devnet.sh
```

If that file doesn't exist, you're on the wrong branch. Stop and check with
the team.

---

## 2. Point the Solana CLI at devnet

```bash
solana config set --url devnet
solana config get
```

The output should include `RPC URL: https://api.devnet.solana.com`. If it
shows mainnet or localhost, the `set` command didn't take — try again.

---

## 3. Find out which wallet you're going to fund

The deploy script uses your default Solana keypair as the fee payer unless
you override it with `DEPLOY_KEYPAIR_PATH`. For the simplest path, just use
the default:

```bash
solana address
```

That prints a base58 pubkey, something like
`9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin`. **Copy this — you'll paste it
into the faucet website in the next step.**

> If `solana address` errors with "no default signer", run
> `solana-keygen new --no-bip39-passphrase` first to create one. Back up the
> seed phrase somewhere safe even though this is just devnet — you'll be
> reusing this keypair if the deploy needs to be re-run.

Check current balance (probably 0):

```bash
solana balance
```

---

## 4. Get devnet SOL from the faucet (the slow part)

You need **at least 4 SOL** in this wallet to deploy. The program is 234 KB
and rent-exempt storage plus deploy buffers eats roughly 3.5 SOL; the extra
0.5 SOL is slack for transaction fees and any retries.

1. Open https://faucet.solana.com/ in a browser.
2. Sign in with **GitHub** (top-right). The signed-in limit is much higher
   than the anonymous one. Use a GitHub account with some commit history;
   brand-new accounts sometimes fail verification.
3. Make sure the network selector says **Devnet** (not Mainnet, not Testnet).
4. Paste the pubkey from step 3 into the address field.
5. Request **2 SOL**. Wait for the confirmation toast.
6. In the terminal, verify it landed:
   ```bash
   solana balance
   ```
   You should see `2 SOL`. If you don't, wait 30 seconds and try again —
   devnet sometimes lags.
7. **Repeat steps 4–6 once more** to get to ≥ 4 SOL total.

If the faucet rejects you (rate limit, GitHub validation failure, "try
again later"), don't panic — try one of these in order:

- Wait 5 minutes and retry. Most rate limits are short.
- Try a different teammate's GitHub account.
- Use **DevnetFaucet.org** as a backup — it has a separate rate-limit pool.
- Last resort: ping the team and we'll switch to a personal Helius or
  QuickNode RPC for the airdrop, which is a 5-minute setup but guaranteed.

Do **not** proceed past this step until `solana balance` shows ≥ 4 SOL.

---

## 5. Sanity-check the build before deploying

This catches anchor/toolchain problems without spending any SOL:

```bash
anchor build --no-idl
ls -lh target/deploy/randomness_beacon.so
```

You should see a file around 234 KB. If the build fails, stop and ask the
team — don't try to fix it yourself, and definitely don't deploy a build
that completed with warnings you don't understand.

Also check the program ID your local build will publish at:

```bash
anchor keys list
```

You'll see a line like `randomness_beacon: <pubkey>`. The currently-deployed
program is at
[`2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH`](https://explorer.solana.com/address/2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH?cluster=devnet).

- **Same pubkey?** Great — `anchor deploy` will push an upgrade in place.
- **Different pubkey?** That's expected on a fresh checkout where you've
  generated your own deploy keypair. You'll be deploying a *new* program
  at *your* address; that's fine for a re-run, but the visualizer's
  `web/public/deploy.json` will get overwritten to point at the new one.
  If that's not what you want, ask before continuing.

---

## 6. Run the deploy

```bash
SKIP_AIRDROP=1 scripts/deploy-devnet.sh
```

`SKIP_AIRDROP=1` is important — it tells the script you funded the wallet
manually and to skip its own airdrop attempt (which would just hit the same
rate limit and fail).

What you'll see:

- The script prints what it's about to do at each step.
- `anchor build --no-idl` (probably cached from step 5).
- `anchor deploy` — this is the slow one. It'll upload the program in
  chunks; expect 30–90 seconds. **Do not Ctrl-C it.** If it stalls for more
  than 3 minutes, then you can interrupt and re-run; the script is
  idempotent.
- A success message with the program ID and Explorer URL.
- `web/public/deploy.json` is written with the deploy info.

If it fails partway through with "insufficient funds", you ran out of SOL
mid-deploy. Top up via the faucet again (you'll need ~2 more SOL), then
re-run the same command. The script picks up where it left off.

---

## 7. Verify the deploy on the Explorer

Open the URL the script printed. For the current live program it's:

```
https://explorer.solana.com/address/2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH?cluster=devnet
```

(If you deployed a fresh program from your own keypair, the URL will use
your program id instead — `cat web/public/deploy.json | jq -r .explorer_url`.)

You should see:
- The program account.
- `Executable: Yes`.
- A recent transaction in the history.

If any of that is wrong, stop and report back before committing anything.

---

## 8. Verify the visualizer picks it up

```bash
cat web/public/deploy.json
```

Should show valid JSON with the program ID and Explorer URL.

Run the static site locally:

```bash
cd web
python3 -m http.server 8000
```

Open http://localhost:8000 in a browser. The hero badge at the top should
read **"Deployed · devnet"** as a clickable link. Click it — it should
open the Explorer page from step 7.

Stop the server with Ctrl-C.

---

## 9. Commit and push

`anchor keys sync` (which the script runs as part of the build) rewrites
`programs/randomness-beacon/src/lib.rs` (`declare_id!`) and `Anchor.toml`
to match your deploy keypair. **Commit those rewrites alongside
`web/public/deploy.json`** — otherwise the program won't recompile at the
deployed address later.

```bash
git add web/public/deploy.json \
        programs/randomness-beacon/src/lib.rs \
        Anchor.toml \
        artifacts/verifying_key_solana.rs
git commit -m "deploy: publish randomness_beacon to devnet"
git push
```

`artifacts/verifying_key_solana.rs` is included because the script may
have regenerated it (only on first deploy or when `FORCE_SETUP=1`); on
re-deploys with the existing artifacts it'll be unchanged and the `git
add` is a no-op.

Open a PR into `main` (or merge directly if the team is comfortable —
the deploy commit on May 10 was a direct push by Cathy).

---

## 10. Tell the team

Drop a message like:

> Deployed to devnet. Program ID `<from web/public/deploy.json>`, explorer
> link in the visualizer hero badge. New deploy tx: `<tx>`.

You're done.

---

## If something goes wrong

| Symptom | What it means | What to do |
|---|---|---|
| Faucet says "rate limited" or "try again later" | Shared rate-limit bucket | Wait 5 min, try a different GitHub account, or use DevnetFaucet.org |
| Faucet says "GitHub validation failed" | Account too new | Use a teammate's older GitHub account |
| `solana balance` shows 0 after faucet success | Devnet RPC lag | Wait 30s, run `solana balance` again |
| `anchor build` fails | Toolchain mismatch | Stop, ping the team — don't fix it ad-hoc |
| `anchor keys list` shows a different program ID | Wrong program keypair locally | Stop, ping the team — do not deploy |
| `anchor deploy` says "insufficient funds" mid-deploy | Ran out of SOL | Top up via faucet, re-run the script (idempotent) |
| Deploy stalls > 3 min | Network hiccup | Ctrl-C, re-run with `SKIP_AIRDROP=1` |
| Visualizer still shows "deploy pending" | Browser cache, or `deploy.json` reverted to placeholder | Hard-refresh the page; if still broken, `cat web/public/deploy.json` to confirm `program_id` is non-null and `status: "deployed"` |
