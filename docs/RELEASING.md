# Releasing CCTV

The release pipeline lives in [`.github/workflows/release.yml`](../.github/workflows/release.yml).
Pushing a `v*` tag builds a **universal** macOS binary (Intel + Apple Silicon),
signs and notarizes it (if the Apple secrets are configured), and creates a
**draft** GitHub Release with the `.dmg` attached.

## TL;DR — cut a release

```bash
# 1. Bump the version in BOTH files (keep them in sync):
#    - package.json            -> "version"
#    - src-tauri/tauri.conf.json -> "version"
# 2. Commit the bump.
git commit -am "chore: release v0.1.0"

# 3. Tag and push. The tag must match the version above.
git tag v0.1.0
git push origin main --tags
```

The workflow runs, and a **draft** release appears under *Releases*. Review the
attached `.dmg`, then click **Publish**.

> The release is a draft on purpose — nothing goes public until you publish it.

## One-time setup: signing & notarization

Without these, the workflow still builds — but the app is **unsigned**, so users
hit Gatekeeper ("CCTV can't be opened because Apple cannot check it for malicious
software"). Signing + notarization removes that. It requires an
[Apple Developer Program](https://developer.apple.com/programs/) membership ($99/yr).

### 1. Create a Developer ID Application certificate

In Xcode → Settings → Accounts → Manage Certificates → **+** → *Developer ID
Application*. (Or create it in the Apple Developer portal and import it into
Keychain.)

### 2. Export and base64-encode the certificate

In **Keychain Access**, find the *Developer ID Application* certificate, right-click
→ **Export** → save as a `.p12`, and set an export password.

```bash
openssl base64 -A -in certificate.p12 -out certificate-base64.txt
```

### 3. Find your signing identity and Team ID

```bash
security find-identity -v -p codesigning
# -> "Developer ID Application: Your Name (TEAMID1234)"
```

The quoted string is your **signing identity**; the 10-char code in parentheses
is your **Team ID**.

### 4. Create an app-specific password (for notarization)

At [appleid.apple.com](https://appleid.apple.com) → Sign-In and Security →
App-Specific Passwords → generate one for "CCTV notarization".

### 5. Add the GitHub repository secrets

Repo → Settings → Secrets and variables → Actions → **New repository secret**.
Add each of these:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Contents of `certificate-base64.txt` |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12` export password from step 2 |
| `APPLE_SIGNING_IDENTITY` | The full quoted identity from step 3, e.g. `Developer ID Application: Your Name (TEAMID1234)` |
| `APPLE_ID` | Your Apple account email |
| `APPLE_PASSWORD` | The app-specific password from step 4 |
| `APPLE_TEAM_ID` | The 10-char Team ID from step 3 |

That's it — the next tagged release will be signed and notarized automatically.

## Installing an unsigned build (interim)

Until signing is set up, users can still run the app:

```bash
xattr -dr com.apple.quarantine /Applications/CCTV.app
```

…or right-click the app → **Open** → **Open** on the first launch. A Homebrew tap
can also pass `--no-quarantine`. None of this is needed once notarization is on.
