# GitHub setup and releases

The workflows are ready for their first hosted run. No GitHub repository, CI run, installer or published release has been created by the agent. The initial CI channel produces **unsigned prereleases**: macOS has an ad-hoc signature but no Developer ID/notarization, and Windows has no Authenticode signature. Signing commands are documented in `packaging/release/README.md`; signing credentials are not configured in these workflows.

## 1. Create and connect the repository

Create an **empty** GitHub repository named `uFFmpeg` under the account or organization you choose. Do not initialize it with another README/license. Choose public only if you intend to publish the source. Standard hosted runner minutes are free for public repositories; private repositories use your account's included minutes.

From this project directory, after reviewing the files:

```sh
cd /Users/muzammilpeer/Git/uFFmpeg
git add .github .gitattributes .gitignore .dockerignore Cargo.toml Cargo.lock LICENSE README.md apps crates assets docs sprint packaging scripts
git diff --cached --stat
git commit -m "Prepare native editor and GitHub release pipeline"
git remote add origin https://github.com/YOUR-ACCOUNT/uFFmpeg.git
git push -u origin main
```

The local repository is initialized on `main`. If copying the source into a different folder without `.git`, run `git init -b main` first. Configure your own Git author name/email if Git requests them. Replace `YOUR-ACCOUNT`; authenticate using GitHub's supported HTTPS credentials or use your SSH remote. Build outputs, work directories, bundled vendor builds, local environment files and signing keys are ignored. Do not commit your media or credentials.

Optional GitHub CLI route (install with `brew install gh`, then `gh auth login`): after the initial commit, run `gh repo create YOUR-ACCOUNT/uFFmpeg --private --source=. --remote=origin --push`. Choose `--public` instead only when you intend public visibility. Do not run this if you already created and connected the repository above.

## 2. Repository settings

- Enable GitHub Actions under **Settings → Actions → General**. Repository/organization policy must allow the actions in `.github/workflows/`. The release jobs request `contents: write`; no personal access token or repository secret is needed. An organization policy can override those permissions.
- Optional: **Settings → Secrets and variables → Actions → Variables**, add `UFFMPEG_MAINTAINER` with your desired Debian package maintainer (`Name <email>`). Otherwise CI uses the repository owner's GitHub no-reply identity.
- Protect `main` and restrict who can create `v*` tags. Releases execute the code at that tag with permission to update release assets. Do not move a released tag.
- Set a GitHub Actions budget with **Stop usage when budget limit is reached** if you want to prevent paid overage. Monitor storage and usage in Billing; quotas apply across your account, not just this repository.

## 3. Check and release

The first push to `main` runs **Native desktop checks** on all three operating systems: formatting, strict Clippy, and a workspace build. Pull requests run these checks too. It creates no temporary downloadable executables and runs no test cases.

After those jobs pass, tag the same commit:

```sh
git tag -a v0.1.0 -m "uFFmpeg 0.1.0"
git push origin v0.1.0
```

**Release installers** starts automatically. The tag must exactly match `[workspace.package].version` in `Cargo.toml` and `APP_VERSION` in `packaging/release/sources.env`. Future releases must update both before tagging. For a manual run, choose **Actions → Release installers → Run workflow**, then select the existing version **tag**, not `main`. The workflow rejects branch dispatches. If needed with GitHub CLI: `gh workflow run release.yml --ref v0.1.0`.

The pipeline:

1. Creates or reuses a **draft prerelease** for the existing tag.
2. Builds the pinned FFmpeg/dav1d/zlib sources and audits capabilities, architectures, shared-library dependencies and checksums.
3. Checks Rust formatting/lints, collects dependency license texts (including omitted texts from recorded upstream commits), vendors locked Rust sources, and builds native installers.
4. Validates installer/source checksums and uploads **directly to the draft Release**, avoiding Actions artifact storage for these large downloads.
5. Requires all three platforms and all nine expected assets before replacing the “build in progress” notice with complete release notes. It never publishes automatically.

| Platform | Installer | Accompanying downloads |
| --- | --- | --- |
| macOS Apple Silicon | `uFFmpeg-0.1.0-macos-arm64-unsigned.dmg` | matching `.source.tar.xz` and `.SHA256SUMS` |
| Windows x64 | `uFFmpeg-0.1.0-windows-x64.exe` | `uFFmpeg-0.1.0-source.tar.gz` and `uFFmpeg-0.1.0-windows-x64.SHA256SUMS` |
| Ubuntu 24.04 x64 baseline | `uffmpeg_0.1.0_amd64.deb` | matching `.source.tar.xz` and `.SHA256SUMS` |

Linux ARM64 packaging remains available locally, but is not in the initial three-runner CI matrix. Intel macOS is not built. Linux needs the declared system desktop libraries; all platforms include their own media tools.

## 4. Publish and attach downloads

**You do not need to attach the generated binaries manually.** Open **Releases**, edit the draft for `v0.1.0`, and confirm all nine files appear. Review the successful workflow and license records, install the packages on clean machines, and exercise import, split/automatic segments, merge, audio, cancel/retry and output reveal. This is a manual release review, not an automated test suite. Keep the prerelease marker for this unsigned initial channel. Click **Publish release** when ready. Published assets then appear as download links on the release page.

If one platform fails, the release stays a draft and may contain partial uploads. Read its failed step and fix the source, then use a new version/tag if the tagged source changed. For a transient failure with unchanged code, use **Re-run failed jobs**; it replaces only that platform's draft assets. Never publish a partial draft. The workflow refuses to alter an already published release.

To attach separately produced files to an existing draft through the website, open the draft editor and drag the installer, its matching source archive and checksum file into the **Attach binaries** area. With GitHub CLI, the equivalent is:

```sh
gh release upload v0.1.0 /absolute/path/to/installer /absolute/path/to/source-archive /absolute/path/to/SHA256SUMS --repo YOUR-ACCOUNT/uFFmpeg
```

Use actual filenames. Do not mix checksums or source archives from different builds. GitHub's automatic “Source code (zip/tar.gz)” links do **not** replace the companion archive containing vendored dependencies and build/license records.

## Free-tier costs and limits

As checked on 29 October 2026, [standard hosted runners are free for public repositories](https://docs.github.com/en/actions/concepts/billing-and-usage). [GitHub Free includes 2,000 monthly minutes and 500 MB of Actions artifact storage for private repositories](https://docs.github.com/en/billing/concepts/product-billing/github-actions); OS billing rates/multipliers and account-wide usage matter. A three-platform Rust/FFmpeg build is substantial, so private-repository builds are not guaranteed to stay within the allowance. The workflows use standard runners, bounded timeouts, two build workers and no Actions artifact uploads/caches. They do not purchase runners, signing services or extra storage.

[Release downloads have no total release-size or bandwidth limit, but each file must be under 2 GiB](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases). The upload script enforces that per-file limit. Direct release uploads avoid retaining a second copy in Actions artifact storage. Apple/Windows signing credentials and any associated costs are separate from free CI.

## Remaining acceptance

Source readiness does not establish a successful release build. The first hosted run must pass, and native installation/UI checks and license review remain required before publication. License collection is evidence gathering, not legal certification. Slint's applicable terms and FFmpeg/source distribution obligations remain documented in `docs/packaging.md`. CI does not claim notarization, signed Windows reputation, compatibility with older Linux distributions, or native UI verification.
