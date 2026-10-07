# Account setup

Connect AniList, MyAnimeList, or both to track episodes automatically when playback launches. Without a connection, local history and playback work as before.

## Register AniList

1. Sign in and open [AniList developer settings](https://anilist.co/settings/developer).
2. Create a client named `ani-tui`. Use `https://github.com/ll931217/ani-tui` as its homepage if requested.
3. Set the redirect URL to **`https://anilist.co/api/v2/oauth/pin`**.
4. Save the client and copy its **client ID**.
5. In a separate terminal, run:

```bash
ani-tui accounts connect anilist YOUR_CLIENT_ID
```

The command opens authorization in your browser and also prints the URL. Approve access, then paste the displayed access token into the hidden terminal prompt. The app verifies your identity before saving the connection. No client secret is required. See [AniList authentication documentation](https://docs.anilist.co/guide/auth/).

## Register MyAnimeList

1. Sign in and open [MyAnimeList API configuration](https://myanimelist.net/apiconfig).
2. Create a **native/desktop app**, named `ani-tui`. Use the fork homepage above if requested.
3. Register **`http://127.0.0.1:8766/callback`** as its redirect URL, exactly as written.
4. Save the app and copy its **client ID**.
5. Run:

```bash
ani-tui accounts connect mal YOUR_CLIENT_ID
```

The app starts a local callback listener before opening authorization in your browser. Approve access; the callback completes login automatically. Keep the command running during authorization. Port 8766 must be available. The app uses PKCE and refreshes expired access tokens. This native client does not require a client secret. See [MAL authorization documentation](https://myanimelist.net/apiconfig/references/authorization).

Client IDs can be shared for configuration. Keep access tokens, refresh tokens, and client secrets private; do not commit them or paste them into issues.

## Use and manage accounts

Press `a` from Home, Details, or Settings. The dialog shows connection status and recent sync results; `r` refreshes status and retries, `j/k` or arrows scroll, and Escape closes it. After connecting from another terminal, press `r` to refresh an already-running TUI.

```bash
ani-tui accounts status
ani-tui accounts retry
ani-tui accounts disconnect anilist
ani-tui accounts disconnect mal
```

Omit the client ID from a connect command to print registration instructions without starting authorization.

## Progress behavior

- Launching E3 records progress 3. It does not verify that the episode was completed or that the external player successfully played the stream.
- Updates are outbound. Remote watchlists and history are not imported into local recommendations or history.
- Remote progress never decreases when replaying earlier episodes. Existing completed status is preserved; a new completed status requires a finished title and reaching its known episode total.
- Failed updates remain in SQLite and retry at startup, on another launch, or with a manual retry. Local playback remains responsive.
- MyAnimeList uses the title's authoritative AniList `idMal` mapping. A missing mapping produces a retryable error rather than guessing a title.
- Jobs are bound to the connected user's ID. Disconnecting or connecting a different user clears that provider's pending jobs.

## Credentials and troubleshooting

Tokens are stored in `accounts.json` alongside `config.toml`: `~/.config/ani-tui/` on Linux and `~/Library/Application Support/ani-tui/` on macOS. The credentials file is separate from preferences and uses owner-only permissions on Unix. Writes are atomic; account mutations and refreshes are locked across processes. Tokens are stored locally, not encrypted with a system keychain.

If AniList authorization expires or is revoked, reconnect. MAL refreshes automatically; reconnect if refresh access has expired or been revoked. Provider outages retain queued updates for retry. Disconnect removes local credentials and pending jobs; revoke authorization in the provider's account settings to remove server-side access too.

The implementation has automated credential, callback, queue, and progress-merge tests. Full live authorization and remote progress writes require registered client IDs and account approval.
