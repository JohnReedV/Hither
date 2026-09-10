#!/usr/bin/env python3
"""Sequential, same-executable visibility comparisons with isolated settings.

Total frame time is the score. GPU timing instrumentation is disabled here.
Use --binary and --asset-root to pin an executable and immutable asset snapshot.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--binary', type=Path, default=ROOT / 'target/release/hither-sdf')
p.add_argument('--asset-root', type=Path, default=ROOT)
p.add_argument('--output', type=Path, default=ROOT / 'tools/build/visibility-benchmark')
p.add_argument('--modes', nargs='+', default=['legacy', 'depth', 'gpu'])
p.add_argument('--cases', nargs='+', choices=['wall', 'blocked', 'forest', 'mountains', 'turn'], default=['blocked', 'forest', 'wall'])
p.add_argument('--resolution', type=int, default=6, help='6=720p, 9=1080p, 12=4K')
p.add_argument('--repeats', type=int, default=1)
p.add_argument('--warmup', type=float, default=12)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
results = []
def other_games(exclude=None):
    found = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit() or int(entry.name) == exclude:
            continue
        try:
            if (entry/'stat').read_text().split(') ', 1)[1].split()[0] in ('T', 't'):
                continue
            executable = (entry/'exe').resolve(strict=True).name
            if executable in ('hither-sdf', 'smoke-binary', 'log-tests', 'rustc', 'rust-lld') or executable.startswith('hither_sdf-'):
                found.append(int(entry.name))
        except (OSError, PermissionError):
            pass
    return found

pattern = re.compile(r'PROFILE (stationary|moving): fps=([\d.]+) median_ms=([\d.]+) p95_ms=([\d.]+) p99_ms=([\d.]+)')
for repeat in range(a.repeats):
    modes = a.modes if repeat % 2 == 0 else list(reversed(a.modes))
    for case in a.cases:
        for mode in modes:
            run = a.output / f'{case}-{mode}-r{a.resolution}-{repeat}'
            config = run / 'config/hither-sdf'
            config.mkdir(parents=True, exist_ok=True)
            (config/'settings.json').write_text(json.dumps(dict(max_fps=1000,show_fps=False,display_mode='windowed',resolution=a.resolution,anti_aliasing='fxaa',render_distance=48,detail_distance=24)))
            env = {k:v for k,v in os.environ.items() if not k.startswith('HITHER_')}
            env.update(HITHER_PROFILE_FOREST='1',HITHER_PROFILE_OFFSCREEN='1',HITHER_PROFILE_NO_GPU_TIMINGS='1',HITHER_PROFILE_SPEED='0',HITHER_PROFILE_WARMUP=str(a.warmup),HITHER_WORLD_SEED='1',HITHER_VISIBILITY_MODE=mode,BEVY_ASSET_ROOT=str(a.asset_root.resolve()),XDG_CONFIG_HOME=str(config.parent.resolve()))
            if case == 'wall':
                env['HITHER_PROFILE_POSE'] = '2.9,1.65,0,1.5707963,0'
            else:
                env['HITHER_PREVIEW_BIOME'] = 'mountains' if case == 'mountains' else 'forest'
            if case in ['blocked','turn']:
                env['HITHER_PROFILE_OCCLUDER'] = '1'
            if case == 'turn':
                env['HITHER_PROFILE_TURN'] = str(0.7853981634)
            log = run/'runtime.log'
            for attempt in range(5):
                while other_games():
                    print('Waiting for another game or compiler to finish.', flush=True)
                    time.sleep(10)
                with log.open('w') as output:
                    process = subprocess.Popen([str(a.binary.resolve())],env=env,stdout=output,stderr=subprocess.STDOUT)
                    start = time.monotonic()
                    overlap = False
                    while process.poll() is None:
                        overlap |= bool(other_games(process.pid))
                        if time.monotonic() - start > a.warmup+100:
                            process.kill()
                            process.wait()
                            raise TimeoutError(str(log))
                        time.sleep(0.5)
                if not overlap:
                    break
                print(f'Discarding {case}/{mode}: another game or compiler ran concurrently.', flush=True)
            else:
                raise RuntimeError('Could not obtain an uncontended rendering run')
            content = log.read_text()
            if process.returncode or re.search(r'ERROR|panicked',content):
                raise RuntimeError(f'Invalid rendering run: {log}\n{content[-5000:]}')
            phases = {m[0]: dict(zip(['fps','median_ms','p95_ms','p99_ms'],map(float,m[1:]))) for m in pattern.findall(content)}
            if len(phases) != 2:
                raise RuntimeError(f'Missing measurements: {log}')
            population = re.search(r'PROFILE instances=(\d+) visible=(\d+) visible_triangles=(\d+)',content)
            item = dict(case=case,mode=mode,resolution=a.resolution,repeat=repeat,phases=phases,population=list(map(int,population.groups())) if population else None,log=str(log))
            results.append(item)
            (a.output/'results.json').write_text(json.dumps(results,indent=2))
            print(f"{case:10} {mode:8} r{a.resolution} #{repeat}: {phases['moving']}",flush=True)
