#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

"""Build portable release archives and native Linux packages without installing them."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def run(*args, **kwargs):
    subprocess.run(args, cwd=ROOT, check=True, **kwargs)


def git(*args):
    result = subprocess.run(['git', *args], cwd=ROOT, text=True, capture_output=True)
    return result.stdout.strip() if result.returncode == 0 else 'unknown'


def checksums(directory):
    files = sorted(p for p in directory.iterdir() if p.is_file() and p.name != 'SHA256SUMS')
    (directory / 'SHA256SUMS').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in files))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=['linux-amd64', 'linux-arm64', 'darwin-amd64', 'darwin-arm64'])
    parser.add_argument('--binary', type=Path, help='Package an already built binary for this platform')
    parser.add_argument('--checksums-only', action='store_true')
    parser.add_argument('--output', type=Path, default=ROOT / 'dist')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.checksums_only:
        checksums(output)
        return
    if not args.platform:
        parser.error('--platform is required')
    system, arch = args.platform.split('-')
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    work = ROOT / 'target' / 'distribution' / args.platform
    work.mkdir(parents=True, exist_ok=True)
    if args.binary:
        binary = args.binary.resolve()
    elif system == 'linux':
        binary = work / 'binary' / 'nysos'
        run('docker', 'buildx', 'build', '--platform', f'linux/{arch}',
            '--file', 'packaging/Dockerfile', '--output', f'type=local,dest={binary.parent}',
            '--build-arg', f'NYSOS_GIT_COMMIT={git("rev-parse", "HEAD")}',
            '--build-arg', f'NYSOS_GIT_TAG={git("tag", "--points-at", "HEAD") or "unknown"}',
            '--build-arg', f'NYSOS_GIT_DESCRIBE={git("describe", "--tags", "--always", "--dirty")}',
            '--build-arg', f'SOURCE_DATE_EPOCH={os.environ.get("SOURCE_DATE_EPOCH", git("show", "-s", "--format=%ct", "HEAD"))}', '.')
    else:
        host_arch = {'arm64': 'arm64', 'aarch64': 'arm64', 'x86_64': 'amd64'}.get(platform.machine())
        if platform.system() != 'Darwin' or host_arch != arch:
            parser.error('macOS archives must be built on the matching macOS architecture')
        run('cargo', 'build', '--locked', '--release')
        binary = ROOT / 'target/release/nysos'
    header = binary.read_bytes()[:32]
    if system == 'linux':
        expected = 62 if arch == 'amd64' else 183
        if header[:4] != b'\x7fELF' or struct.unpack('<H', header[18:20])[0] != expected:
            raise SystemExit('Binary does not match the requested Linux architecture')
    else:
        expected = 0x1000007 if arch == 'amd64' else 0x100000c
        if header[:4] != b'\xcf\xfa\xed\xfe' or struct.unpack('<I', header[4:8])[0] != expected:
            raise SystemExit('Binary does not match the requested macOS architecture')
    completion_dir = work / 'completions'
    run('cargo', 'run', '--locked', '--example', 'generate_completions', '--', str(completion_dir))
    stage = work / 'root'
    if stage.exists():
        shutil.rmtree(stage)
    payload = {
        binary: 'bin/nysos',
        ROOT / 'docs/man/nysos.1': 'share/man/man1/nysos.1',
        completion_dir / 'nysos.bash': 'share/bash-completion/completions/nysos',
        completion_dir / '_nysos': 'share/zsh/site-functions/_nysos',
    }
    for name in ['LICENSE', 'NOTICE', 'README.md']:
        payload[ROOT / name] = f'share/doc/nysos/{name}'
    sbom = ROOT / 'sbom/nysos.spdx.json'
    packages = json.loads(sbom.read_text())['packages']
    if not any(p.get('name') == 'nysos' and p.get('versionInfo') == version for p in packages):
        raise SystemExit('SBOM does not describe this nysos version; run make sbom')
    payload[sbom] = 'share/doc/nysos/nysos.spdx.json'
    for example in (ROOT / 'examples').glob('*.toml'):
        payload[example] = f'share/nysos/examples/{example.name}'
    for source, relative in payload.items():
        dest = stage / relative
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, dest)
        dest.chmod(0o755 if relative == 'bin/nysos' else 0o644)
    if system == 'darwin':
        installed_binary = stage / 'bin/nysos'
        linked = subprocess.check_output(['otool', '-L', str(installed_binary)], text=True)
        for line in linked.splitlines()[1:]:
            library = line.strip().split(' (', 1)[0]
            if library.startswith('/nix/store/'):
                if Path(library).name != 'libiconv.2.dylib':
                    raise SystemExit(f'Unbundled Nix runtime dependency: {library}')
                # Nix's Darwin libiconv is Apple's library, also supplied by macOS.
                run('install_name_tool', '-change', library, '/usr/lib/libiconv.2.dylib', str(installed_binary))
        run('codesign', '--force', '--sign', '-', str(installed_binary))
        subprocess.run([str(installed_binary), '--demo', '--check'], check=True)
    name = f'nysos_{version}_{args.platform}'
    with tarfile.open(output / f'{name}.tar.gz', 'w:gz') as archive:
        for child in sorted(stage.iterdir()):
            archive.add(child, arcname=child.name)
    if system == 'linux':
        env = dict(os.environ, NYSOS_PACKAGE_ARCH=arch, NYSOS_PACKAGE_VERSION=version, NYSOS_PACKAGE_ROOT=str(stage))
        template = (ROOT / 'packaging/nfpm.yaml').read_text()
        for key in ['NYSOS_PACKAGE_ARCH', 'NYSOS_PACKAGE_VERSION', 'NYSOS_PACKAGE_ROOT']:
            template = template.replace('${' + key + '}', env[key].replace("'", "''"))
        config = work / 'nfpm.yaml'
        config.write_text(template)
        for kind in ('deb', 'rpm'):
            run('nfpm', 'package', '--config', str(config), '--packager', kind,
                '--target', str(output / f'{name}.{kind}'), env=env)
    print(f'Created release artifacts for {args.platform} in {output}')


if __name__ == '__main__':
    main()
