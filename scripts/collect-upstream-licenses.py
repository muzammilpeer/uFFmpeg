#!/usr/bin/env python3
"""Collect omitted crate license texts from exact published upstream revisions."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
from urllib.parse import urlparse


def run(*args):
    return subprocess.check_output(args, text=True)


def license_files(root):
    for path in root.iterdir():
        if path.is_symlink():
            continue
        if path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'AUTHORS')):
            yield path


def has_license(root, depth=3):
    for path in root.iterdir():
        if path.is_symlink():
            continue
        if path.is_file() and (path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING')) or root.name.lower() == 'licenses'):
            return True
        if path.is_dir() and depth and path.name not in ('.git', 'target', 'work', 'dist') and has_license(path, depth - 1):
            return True
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('target')
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    metadata = json.loads(run('cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', args.target))
    nodes = {node['id']: node['dependencies'] for node in metadata['resolve']['nodes']}
    reachable = set(metadata['workspace_members'])
    pending = list(reachable)
    while pending:
        for dependency in nodes.get(pending.pop(), []):
            if dependency not in reachable:
                reachable.add(dependency)
                pending.append(dependency)
    for package in metadata['packages']:
        if package['id'] not in reachable:
            continue
        root = Path(package['manifest_path']).parent
        if not package['source'] or package['license_file'] or has_license(root):
            continue
        key = f"{package['name']}-{package['version']}"
        vcs = json.loads((root / '.cargo_vcs_info.json').read_text())
        revision = vcs['git']['sha1']
        if not re.fullmatch('[0-9a-f]{40}', revision):
            raise ValueError(f'{key}: invalid upstream revision')
        url = urlparse(package['repository'] or package['homepage'] or '')
        parts = url.path.strip('/').split('/')
        if url.hostname != 'github.com' or len(parts) < 2 or not all(re.fullmatch(r'[A-Za-z0-9_.-]+', p) for p in parts[:2]):
            raise ValueError(f'{key}: requires a reviewed upstream license location')
        repo = '/'.join(parts[:2]).removesuffix('.git')
        checkout = args.output / '.repos' / (repo.replace('/', '_') + '-' + revision)
        if not checkout.exists():
            checkout.mkdir(parents=True)
            run('git', 'init', '-q', str(checkout))
            run('git', '-C', str(checkout), 'fetch', '--depth', '1', f'https://github.com/{repo}.git', revision)
            run('git', '-C', str(checkout), 'checkout', '--detach', 'FETCH_HEAD')
            if run('git', '-C', str(checkout), 'rev-parse', 'HEAD').strip() != revision:
                raise ValueError(f'{key}: revision mismatch')
        source = (checkout / vcs.get('path_in_vcs', '')).resolve()
        checkout = checkout.resolve()
        if not source.is_relative_to(checkout):
            raise ValueError(f'{key}: invalid upstream crate path')
        destination = args.output / key
        destination.mkdir()
        # Include crate-specific and inherited workspace license/notice files.
        while True:
            level = source.relative_to(checkout).as_posix().replace('/', '_')
            for path in license_files(source):
                target = destination / level / path.name
                target.parent.mkdir(parents=True, exist_ok=True)
                if path.is_dir():
                    shutil.copytree(path, target, symlinks=True)
                else:
                    shutil.copyfile(path, target)
            if source == checkout:
                break
            source = source.parent
        (destination / 'PROVENANCE.txt').write_text(f'Crate: {key}\nUpstream: https://github.com/{repo}\nRevision: {revision}\nLicense expressions remain those declared by the crate.\n')
        if not has_license(destination):
            raise ValueError(f'{key}: no upstream license text found; review required')
        print(f'Collected upstream licenses: {key}', flush=True)
    # Only license evidence is retained; no upstream checkout is distributed here.
    shutil.rmtree(args.output / '.repos', ignore_errors=True)


if __name__ == '__main__':
    main()
