#!/usr/bin/env python3
"""Repeatable whole-renderer audit benchmark; no swapchain or GPU timer overhead."""
import argparse, json, os, re, subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--asset-root', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--repeats', type=int, default=2)
p.add_argument('--resolution', type=int, default=12)
p.add_argument('--cases', nargs='+', default=['castle', 'castle-low', 'forest', 'boreal', 'mountains'])
p.add_argument('--light-count', type=int, choices=range(513), metavar='0..512', default=0,
               help='Add generic emitter fixtures; use --cases lighting for a fixed view.')
p.add_argument('--shadow-quality', choices=['low', 'medium', 'high'],
               help='Override quality for matched lighting-budget measurements.')
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
pattern = re.compile(r'PROFILE (stationary|moving): fps=([\d.]+) median_ms=([\d.]+) p95_ms=([\d.]+) p99_ms=([\d.]+)')
results = []
for repeat in range(a.repeats):
    for case in a.cases:
        run = a.output / f'{case}-{repeat}'
        config = run/'config/hither-sdf'
        config.mkdir(parents=True, exist_ok=True)
        castle = case.startswith('castle') or case == 'lighting'
        settings = dict(max_fps=1000,show_fps=False,display_mode='windowed',resolution=a.resolution,anti_aliasing='fxaa',texture_quality='high',shadow_quality='low' if case.endswith('low') else 'high',grass_blades=True,render_distance=48 if castle else 1024 if case.endswith('-far') else 512 if case=='mountains' else 260,detail_distance=24 if castle else 35)
        if a.shadow_quality: settings['shadow_quality'] = a.shadow_quality
        (config/'settings.json').write_text(json.dumps(settings))
        env = {k:v for k,v in os.environ.items() if not k.startswith('HITHER_')}
        env.update(HITHER_PROFILE_FOREST='1',HITHER_PROFILE_OFFSCREEN='1',HITHER_PROFILE_NO_GPU_TIMINGS='1',HITHER_PROFILE_WARMUP='20',HITHER_WORLD_SEED='721',BEVY_ASSET_ROOT=str(a.asset_root.resolve()),XDG_CONFIG_HOME=str(config.parent.resolve()))
        if a.light_count: env['HITHER_LIGHTING_TEST'] = str(a.light_count)
        if castle:
            env.update(HITHER_PROFILE_POSE='0,1.65,3,0,0',HITHER_PROFILE_SPEED='0')
        else:
            env.update(HITHER_PREVIEW_BIOME=case.split('-')[0],HITHER_PROFILE_SPECTATOR='1',HITHER_PROFILE_SPEED='40',HITHER_PROFILE_HEIGHT='160' if case=='mountains' else '24' if case.startswith('forest') else '8')
        log = run/'runtime.log'
        with log.open('w') as f:
            child = subprocess.run([str(a.binary.resolve())], env=env, stdout=f,
                                   stderr=subprocess.STDOUT, timeout=180)
        content = log.read_text()
        if child.returncode or re.search(r'ERROR|panicked',content): raise RuntimeError(f'Invalid run {log}\n{content[-4000:]}')
        phases = {m[0]:dict(zip(['fps','median_ms','p95_ms','p99_ms'],map(float,m[1:]))) for m in pattern.findall(content)}
        if len(phases)!=2: raise RuntimeError(f'Missing timings: {log}')
        item=dict(case=case,repeat=repeat,light_count=a.light_count,settings=settings,phases=phases,log=str(log))
        results.append(item)
        (a.output/'results.json').write_text(json.dumps(results,indent=2))
        print(json.dumps(item),flush=True)
