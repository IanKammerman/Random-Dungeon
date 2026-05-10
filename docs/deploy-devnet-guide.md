# Deploying `randomness_beacon` to Solana devnet — step-by-step

**Audience:** the teammate who's actually going to run the deploy.
**Time:** 15–25 minutes including faucet waits.
**Cost:** $0. Devnet SOL has no monetary value and the faucets are free.
**Prerequisite knowledge:** you can open a terminal. That's it.

---

## 0. Before you start

You need:

- A clone of the repo on the `mvp-final-prep` branch (or whatever branch PR
  #8 has been merged into).
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
git checkout mvp-final-prep
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

Also verify the program ID matches what the team expects:

```bash
anchor keys list
```

Look for a line like:
```
randomness_beacon: 5MMjTfc64Q9AC2rjVda1ZHH137TebNpdUzNhTMg7Vypx
```

If the pubkey is **different**, stop. That means the program keypair on
your machine isn't the one the team committed to, and deploying would
publish the program at the wrong address. Ask before continuing.

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

Open the URL the script printed — it'll look like:

```
https://explorer.solana.com/address/5MMjTfc64Q9AC2rjVda1ZHH137TebNpdUzNhTMg7Vypx?cluster=devnet
```

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
now read **"Deployed · devnet"** as a clickable link (instead of the
"Devnet deploy pending" state). Click it — it should open the Explorer
page from step 7.

Stop the server with Ctrl-C.

---

## 9. Commit and push

```bash
git add web/public/deploy.json
git commit -m "deploy: publish randomness_beacon to devnet"
git push
```

Open a PR into `mvp-final-prep` (or merge directly if the team is
comfortable — it's one file).

---

## 10. Tell the team

Drop a message like:

> Deployed to devnet. Program ID `5MMjTfc64Q9AC2rjVda1ZHH137TebNpdUzNhTMg7Vypx`,
> explorer link in `web/public/deploy.json`. Visualizer hero badge is live.

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
| Visualizer still shows "deploy pending" | Browser cache, or `deploy.json` not written | Hard-refresh the page; if still broken, `cat web/public/deploy.json` to confirm it exists |
