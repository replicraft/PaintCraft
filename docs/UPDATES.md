# Desktop updates

PaintCraft has desktop update checks and signed downloads in **Settings > About > Updates**. Checks, downloads and installation start only when you choose them.

Install the first update-capable build manually from [Releases](https://github.com/replicraft/PaintCraft/releases). Published v0.1.1 downloads have no updater. If your copy has no Updates controls, download a newer complete installation first. These instructions describe the implemented update flow; they do not announce a new release.

## Check, download and restart

1. Open **Settings > About** and choose **Check for updates**. **Release notes** opens the release page.
2. If an update is available for this installation, choose **Download update**. PaintCraft verifies its signature before showing **Verified update ready**. You can cancel a check or download; cancellation may wait for the current network operation to return.
3. Finish open dialogs, printing, clipboard, paste, export and font-loading operations. Save changes in the other interface if you have a parked Classic or Studio workspace. Close other PaintCraft editor processes using this installation.
4. Choose **Restart to update**, then **Save all and restart**. PaintCraft uses the normal save, Save As and overwrite prompts for modified documents. Cancelling or failing a save leaves the documents open and stops the update. There is no discard-and-update shortcut.
5. PaintCraft stops background work before installing the verified update and requesting a restart. If it asks you to close PaintCraft after an installation error, close it before trying again.

Cancelling the restart prompt keeps unapplied settings and open documents. Once background shutdown or installation has begun, use the displayed close/restart instructions. A refused or failed installation is reported rather than treated as successful.

## Which installations can update themselves?

| Installation | Update behavior |
| --- | --- |
| macOS `PaintCraft.app` | Replaces the complete app, including its launcher, nested editor and codec helper. Copy the app off its DMG into a writable location first. Read-only or translocated copies cannot be replaced in place. |
| Windows MSI | Downloads the matching MSI and asks Windows Installer to update after the editor exits. Windows may request administrator approval. |
| Windows portable | Replaces the matched editor, codec helper, manifest and notices together. |
| Linux AppImage | Replaces the verified AppImage from which PaintCraft was launched. |
| Flatpak, DEB, RPM, tar archive, development or unrecognized installation | Offers the release page. Use the package manager or install a new copy; PaintCraft does not overwrite these installations. |

The browser app receives updates through its website deployment. These desktop controls do not install browser updates.

## Recovery and other editor processes

New update-capable editor processes coordinate access to the same installation. Close all other editors before installing. Older published builds, including the v0.1.1 downloads, do not participate in that coordination and must also be closed manually. This coordination does not discover every older or separately logged-in editor process.

The updater keeps originals for recovery when replacing portable files, an app bundle or an AppImage. Replacement failures and failures to start the restart helper attempt rollback; a rollback failure is reported. Backups are not automatically removed. Starting a new process is not a health check: PaintCraft does not automatically detect a later startup failure or restore the old version after one. Windows Installer controls MSI recovery separately.

## Signing releases

The trusted public key is **`packaging/paintcraft-update-key.pub`**. Desktop builds embed it; a downloaded key cannot replace that trust. Key rotation requires an update authenticated by the old key that embeds the reviewed replacement key, or a manual installation.

The protected GitHub environment **`release-signing`** supplies:

- **`UPDATE_SIGNING_KEY`**: the complete Minisign secret-key text.
- **`UPDATE_SIGNING_PASSWORD`**: the password for the encrypted key.

Private keys and passwords belong in GitHub secrets, never in the repository. The signing script stages its temporary key with private permissions, passes the password through standard input and suppresses subprocess diagnostics.

After finalizing the release files, the release workflow signs every payload and `SHA256SUMS.txt`. Each file gets a `.minisig` sidecar with this exact authenticated comment:

```text
paintcraft-update VERSION ASSET_NAME
```

Replace `VERSION` below with the workspace version. Public verification needs no signing secrets:

```sh
python scripts/packaging/sign_release.py --version VERSION --assets dist --public-key packaging/paintcraft-update-key.pub --verify-only
```

Release downloads use Minisign authentication. macOS Developer ID/notarization and Windows Authenticode remain separate packaging work; current macOS bundles use ad-hoc signatures. See [release packaging and signatures](RELEASING.md#update-signatures).

## Validation status

Local tests cover signature and version checks, bounded downloads and extraction, matched helper files, rollback, installation coordination, actual native save queues, cancellation and all seven update translations. The save tests preserve dirty documents and existing destination bytes on failure or cancellation.

A full signed update through the live desktop UI remains unqualified. Actual Windows/MSI installation, elevation and relaunch, cross-platform restart behavior, and real Linux Flatpak runtime integration still need platform validation. Passing local component tests does not establish those outcomes. Release packaging checks and remaining platform limits are recorded in [Releasing](RELEASING.md).
