# Note-inator

A small, fast, password-protected note-taking app for Windows. Notebooks → Sections → Pages, rich-text
editing (TipTap), and a single encrypted vault file. Built with Tauri 2 (Rust backend, Svelte frontend).
A CAi (Cursed Assistant-inator) product. The app icon (`src-tauri/icons/icon.svg`, a torn-out notebook page that
hides "Ni") lives with the CAi logo assets under `public/branding/`; colors follow the CAi tokens in `src/app.css`.

The app was originally called SecureNotes. Its data folders (`%APPDATA%\SecureNotes`, `%LOCALAPPDATA%\SecureNotes`)
and the `securenotes.exe` file name are kept so existing vaults and taskbar pins keep working.

> ## ⚠️ There is no password recovery
> Your notes are encrypted with a key derived from your password. **If you forget the password, the notes
> cannot be recovered by anyone** — there is no reset, no backdoor, and no recovery key. Store your password
> somewhere safe (for example a password manager) and keep a backup of the vault (see below).

## Security design

| Item | Detail |
| --- | --- |
| Vault file | `%APPDATA%\SecureNotes\vault.dat` (one file, entirely encrypted apart from a small header) |
| Key derivation | Argon2id, random 16-byte per-vault salt. Parameters are calibrated when the vault is created (and when the password changes) to take about 0.75 s on that machine: 256 MiB memory, 4 lanes, tuned pass count. Parameters are stored in the header. |
| Encryption | XChaCha20-Poly1305, fresh random 24-byte nonce on **every** save |
| Integrity | The whole header (magic, version, KDF parameters, salt, nonce) is authenticated as AEAD associated data. A wrong password or any modification fails with: "Incorrect password, or the vault file has been modified or damaged." |
| Crash safety | Saves write `vault.dat.tmp`, flush it to disk, keep the previous version as `vault.dat.bak`, then rename the new file into place. `vault.dat` is always a complete old or new file. |
| Key handling | The derived key exists only in the Rust process while unlocked and is zeroized (`zeroize` crate) on lock and exit |
| Crypto crates | `argon2`, `chacha20poly1305` (RustCrypto). No custom cryptography. |

File layout: `"SNVT"` · version u16 · KDF id u8 · m u32 · t u32 · p u32 · salt[16] · nonce[24] · ciphertext+tag.

### What "nothing in plaintext on disk" does and doesn't cover

Note-inator never writes note content to disk unencrypted: no temp files, caches, logs, or unencrypted
autosave. Things outside the app's control that you should know about:

- **Windows itself**: the pagefile and hibernation file can contain whatever was in memory, including
  decrypted notes. Full-disk encryption (BitLocker) is the remedy.
- **Clipboard**: text you copy from a note goes to the Windows clipboard (and clipboard history, Win+V).
- **Memory**: the key is zeroized on lock, but decrypted note text lives in ordinary memory (Rust strings
  and the WebView's JavaScript heap) and is only released, not overwritten, on lock.
- **WebView2 profile**: the embedded browser keeps its profile in `%LOCALAPPDATA%\SecureNotes\webview`;
  notes never use web storage, and the folder is deleted on startup and exit.
- Settings (auto-lock, theme, …) are stored *inside* the encrypted vault.

## Using the app

- **First run**: choose a password (twice, minimum 8 characters) and acknowledge there is no recovery.
- **Lock**: the 🔒 button or `Ctrl+L`. Locking saves pending changes, drops the key, clears the UI, and minimizes the window.
- **Auto-lock** (Settings): after 1–60 minutes idle (default 5, or off); always when Windows locks or
  sleeps; optionally when the window is minimized.
- **Organizing**: `＋` creates notebooks / sections / pages; double-click or ✎ renames; 🗑 deletes (with
  confirmation). Drag to reorder at every level; drag a page onto a section to move it.
- **Editor**: headings, bold/italic/underline/strikethrough, text color and highlight, bulleted /
  numbered / checkbox lists, tables, links, code blocks, horizontal rules. Pasted content from OneNote,
  Word, or browsers keeps supported formatting; images and anything else unsupported are dropped.
  Edits autosave (encrypted) 3 seconds after you stop typing.

## Backing up and restoring

The vault is one file and is always encrypted, so copying it is safe.

- **Export backup** (Settings → Backup): copies the encrypted vault to a location you choose.
- **Import backup**: replaces the current vault with a chosen backup (the current one is kept as
  `vault.dat.bak`) and locks. You'll need the *backup's* password. The first-run screen also offers
  "Restore from a backup", which is handy on a new PC.
- **Manual**: copy `%APPDATA%\SecureNotes\vault.dat` yourself. If it is ever damaged, the previous save is
  `vault.dat.bak` in the same folder (rename it to `vault.dat` with the app closed).

Back up regularly and keep copies off the machine.

## Building

Requirements: Rust (stable), Node.js 22+, and a linker for your Rust target:
either the Visual Studio C++ Build Tools ("Desktop development with C++") for the standard
`x86_64-pc-windows-msvc` target, or a MinGW-w64 toolchain on `PATH` for `x86_64-pc-windows-gnu`.
WebView2 ships with Windows 11. The first bundle step downloads NSIS automatically.

```powershell
npm install
npm run tauri build      # installer: src-tauri\target\release\bundle\nsis\Note-inator_0.1.0_x64-setup.exe
```

Development: `npm run tauri dev`. Storage-layer tests: `cd src-tauri; cargo test`.

The installer is a per-user NSIS installer (no administrator rights needed). Uninstalling does **not**
delete your vault in `%APPDATA%\SecureNotes`.

## Project layout

```
src/                 Svelte UI (Setup, Unlock, Main, Editor, Settings, dialogs)
src-tauri/src/
  vault/             crypto.rs, format.rs, store.rs (+ tests), model.rs, error.rs
  state.rs           locked/unlocked state, idle timer, lock requests
  commands.rs        Tauri commands (thin layer over the vault)
  winevents.rs       lock on Windows session-lock / sleep
```
