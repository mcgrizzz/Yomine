<!-- BETA_INCLUDE_START -->
**⚠️ This is a beta release**

This is a pre-release version intended for testing and feedback. It may contain bugs and is not recommended for the average user.

Please report any issues you encounter to help us improve the final release.
<!-- BETA_INCLUDE_END -->

---

<!-- CHANGELOG_INSERTION_POINT -->

## Downloads

- **Windows:** the `-setup.exe` installer *(recommended)*, or the `.msi`
- **macOS:** the `.dmg` *(universal: Intel + Apple Silicon)*
- **Linux:** the `.AppImage`, `.deb`, or `.rpm`

**Already have Yomine installed?** It checks for updates on launch and can install this version in-app — no download needed.

<details>
<summary><strong>Installation notes</strong></summary>

**Windows:** Run the setup `.exe`. If SmartScreen warns about an unknown publisher, click *More info → Run anyway*.

**macOS:** Open the `.dmg` and drag Yomine to Applications. The app isn't notarized yet, so if macOS refuses to open it, clear the quarantine flag:
```bash
xattr -cr /Applications/Yomine.app
```

**Linux:** Install the `.deb`/`.rpm` with your package manager, or make the AppImage executable and run it:
```bash
chmod +x Yomine_*.AppImage && ./Yomine_*.AppImage
```

📋 **Verification:** SHA256 checksums for all files are in `SHA256SUMS.txt`.

ℹ️ `latest.json` and the `.sig`/`.tar.gz` files power the in-app updater — you can ignore them.

</details>

---

*Happy mining! ⛏️*
