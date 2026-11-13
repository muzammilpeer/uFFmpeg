#!/usr/bin/env python3
"""Validate release inputs and attach CI outputs to an unpublished GitHub Release."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


def gh(*args):
    return subprocess.check_output(['gh', *args], text=True)


def main():
    mode = sys.argv[1]
    tag = os.environ.get('GITHUB_REF_NAME', '')
    if os.environ.get('GITHUB_REF_TYPE') != 'tag' or not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise ValueError('Run this workflow on a version tag such as v0.1.0, not a branch.')
    version = tag[1:]
    sources = dict(line.split('=', 1) for line in Path('packaging/release/sources.env').read_text().splitlines() if '=' in line and not line.startswith('#'))
    cargo = Path('Cargo.toml').read_text().split('[workspace.package]', 1)[1].split('\n[', 1)[0]
    if sources['APP_VERSION'] != version or not re.search(r'^version\s*=\s*"' + re.escape(version) + r'"\s*$', cargo, re.M):
        raise ValueError('Tag, Cargo workspace version and sources.env APP_VERSION must match.')
    if mode == 'validate':
        print(f'Validated {tag}')
        return
    repo = os.environ['GITHUB_REPOSITORY']
    run_url = f"https://github.com/{repo}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
    expected = {
        'macos': [f'uFFmpeg-{version}-macos-arm64-unsigned.{s}' for s in ('dmg', 'source.tar.xz', 'SHA256SUMS')],
        'linux': [f'uffmpeg_{version}_amd64.{s}' for s in ('deb', 'source.tar.xz', 'SHA256SUMS')],
        'windows': [f'uFFmpeg-{version}-windows-x64.exe', f'uFFmpeg-{version}-source.tar.gz', f'uFFmpeg-{version}-windows-x64.SHA256SUMS'],
    }
    if mode == 'prepare':
        result = subprocess.run(['gh', 'release', 'view', tag, '--json', 'isDraft'], capture_output=True, text=True)
        if result.returncode:
            gh('release', 'create', tag, '--verify-tag', '--draft', '--prerelease', '--title', f'uFFmpeg {version}', '--notes', f'Build in progress. Do not publish until all jobs pass.\n\n{run_url}')
    release = json.loads(gh('release', 'view', tag, '--json', 'isDraft,assets,url'))
    if not release['isDraft']:
        raise ValueError('Refusing to modify an already published release. Use a new version tag.')
    if mode == 'upload':
        platform = sys.argv[2]
        directory = Path('dist/windows-unsigned' if platform == 'windows' else 'dist')
        if platform == 'windows':
            (directory / 'SHA256SUMS').rename(directory / expected[platform][-1])
        manifest = directory / expected[platform][-1]
        hashed = set()
        for line in manifest.read_text(encoding='utf-8-sig').splitlines():
            digest, name = line.split('  ', 1)
            if name not in expected[platform][:-1] or name in hashed:
                raise ValueError('Unexpected or duplicate checksum entry')
            with (directory / name).open('rb') as stream:
                actual = hashlib.file_digest(stream, 'sha256').hexdigest()
            if actual != digest.lower():
                raise ValueError(f'Checksum mismatch: {name}')
            hashed.add(name)
        if hashed != set(expected[platform][:-1]):
            raise ValueError('Missing installer or source checksum')
        files = [directory / name for name in expected[platform]]
        if any(p.is_symlink() or not 0 < p.stat().st_size < 2 * 1024**3 for p in files):
            raise ValueError('Release files must be regular, nonempty and under 2 GiB')
        gh('release', 'upload', tag, *(str(p) for p in files), '--clobber')
    elif mode == 'finish':
        required = {name for group in expected.values() for name in group}
        assets = {asset['name'] for asset in release['assets']}
        if assets != required:
            raise ValueError(f'Release assets differ from expected set: {assets ^ required}')
        notes = f'''Native Rust + Slint video editor: split, automatic fixed-duration segments, merge, audio extraction and track selection, queued jobs with cancellation and recovery.

Built from `{os.environ['GITHUB_SHA']}`. [Successful build and audit logs]({run_url}).

- macOS: Apple Silicon DMG, ad-hoc signed; **not Developer ID signed or notarized**.
- Windows: x64 installer, **not Authenticode signed**; Windows may show reputation warnings.
- Linux: x64 Debian package built on Ubuntu 24.04; system desktop libraries are package dependencies.
- Every installer bundles pinned FFmpeg and FFprobe. No separate FFmpeg installation is needed.
- Download each installer with its companion source archive and SHA256SUMS. Source archives include the locked Rust dependency sources and license records; helper sources/licenses are included in the installed application.

All platform CI jobs completed. This remains a draft prerelease until the maintainer reviews license obligations (including Slint), installs it on clean machines, and publishes it. CI diagnostics do not verify the graphical interface or every media operation.
'''
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'notes.md'
            path.write_text(notes)
            gh('release', 'edit', tag, '--notes-file', str(path))
    elif mode != 'prepare':
        raise ValueError('Unknown release operation')
    with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
        summary.write(f"\nDraft release: {release['url']}\n")


if __name__ == '__main__':
    main()
