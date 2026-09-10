# Goblin-den performance and navigation

These FPS results predate the subsequent point-shadow cache in `POINT_SHADOW_CACHE.md`; no FPS claim is made for that later change.

The retained changes reduce navigation stalls and modestly improve the measured interior; overview FPS is unchanged. Point-light shadow work remains the dominant den cost. Only goblin-den benchmarks were run. This is not a claim that all den lag is eliminated.

## Reproduction and attribution

Seed 42, the 20-resident den, interior preview. World rendering is 3840 × 2160, FXAA, High textures/shadows, 260 m render distance, 35 m detail and shadow distances. Offscreen presentation is 1280 × 720 with the existing two-submission GPU completion bound. Each run warms up for 60 seconds, then records two eight-second stationary windows (`speed=0`; the profiler calls the second window `moving`). Builds never overlap FPS collection. There are no CPU-idle admission checks or load-based exclusions.

The reference baseline is review 8's frozen `final-v2`, SHA-256 `d31391548b69eebc562847981aab7b082af2db2d8888bd0ad51c4c0accaf5d8e`.

| Original release intervention | Window 1 FPS | Window 2 FPS |
|---|---:|---:|
| Normal | 121.5 | 115.3 |
| Freeze resident simulation | 129.0 | 125.9 |
| Hide residents, keep simulation | 118.3 | 115.5 |
| Disable point shadows in render world | 220.5 | 205.9 |

Point-shadow attribution preserves main-world light admission, ownership and intensity. It includes render-world shadow preparation, drawing and sampling, and does not remove main-world light visibility work. Residents wander, so these are scene comparisons rather than pixel-identical simulation replays.

A separate, consistently compiled non-LTO diagnostic binary separates costs:

| Diagnostic intervention | Window 1 FPS | Window 2 FPS |
|---|---:|---:|
| Normal | 102.4 | 97.4 |
| Skip point-map clear/draw, retain prior maps and sampling | 127.6 | 119.5 |
| Disable point-shadow sampling, retain map generation | 101.2 | 100.2 |
| Omit procedural terrain/rock shadow casters | 101.9 | 104.5 |

Sampling was not the limiting cost in this scene. Freezing map drawing improves performance while leaving queue/batch preparation running; the broader point-shadow ablation also removes that render preparation. Frozen maps are diagnostic only and are never enabled in normal play. No shadow filtering or lamp-quality reduction is shipped.

## Den mesh grouping

The original den renderer partitions geometry into 32 m spatial bins, then chunks each bin into at most 4,000 triangles. This is too coarse for 14 m lamps: many unrelated triangles share bounds and become shadow candidates on multiple cubemap faces.

| Spatial cell size | Shadow candidate instances across faces | Native shadow candidate triangles | Window 1 / 2 FPS |
|---|---:|---:|---:|
| 32 m | 1,931 | 6,637,925 | 101.4 / 110.4 |
| 4 m | 2,557 | 3,115,121 | 113.3 / 113.9 |
| 8 m | 1,706 | 4,005,400 | 121.1 / 121.5 |

These three runs use the same non-LTO binary. Eight-metre groups balance culling against per-instance overhead. Counts describe CPU-visible shadow candidates, not a hardware count of rasterized triangles. All 24 shadow faces, every authored triangle, vertex attribute, material, lamp and resident remain. Actual mesh bounds include triangles crossing cell boundaries. Tests compare every triangle's attributes and winding bit-for-bit after partitioning.

## Navigation findings and changes

Orcs and goblins share the same navigation core, but previously used different policies. Goblins used long direct shortcuts (up to 16 m), while orc pursuit limits direct corridors to 1 m. Search initialization also traced all nine neighboring columns synchronously before the budgeted search loop. A measured goblin steering call took 8.818 ms despite the nominal 1 ms shared planning allowance.

During a 15-second warmup interval, the old behavior made 9,000 settling calls (181.055 ms total), 6,324 steering calls (821.754 ms total), and 6,155 swept-walk calls (1,075.829 ms total). The measured interval had zero unsupported settling results. Thus overlap recovery was not responsible for the steady-state lag in this particular scene.

The implementation changes are:

- Goblins use short-corridor pursuit with a verified same-floor prefix even when another floor exists above or below them. This avoids waiting for a full search before ordinary movement across a room. Height-changing goals and blocked prefixes still use full supported A* paths.
- Initial search attachment retains its column/floor cursor and resumes within subsequent planning slices. The budget checks now cover initialization, not just graph expansion.
- Overlap recovery retains its cursor, uses at most eight candidate probes per goblin and 64 across the population per simulation tick, rotates admission, and backs off after exhausting all candidates. Corrections retain the original expanding-capsule and floor sweeps. A newly cleared obstacle releases immediately.
- Goblins do not plan or walk until settling reports valid support, matching the orc controller.
- A rejected direct movement step tries at most two axis slides, each using the full supported body sweep and crowd checks. Normal movement still uses one sweep. A regression test verifies that a small resident detours around a tight corner on a stacked floor instead of repeatedly stopping there.
- Accepted actor positions are reserved immediately for subsequent crowd checks, matching the orc ordering and preventing two movers from claiming the same space.

The intermediate short-corridor candidate reduced the maximum observed steering call from 8.818 to 1.868 ms, but made fewer walking calls. The final same-floor prefix and slide refinement passes the corner regression and shows active walking in native captures. Final interior warmup intervals recorded 6,324 steering calls in 524.859 / 529.712 ms total, with maxima of 2.012 / 2.135 ms. These are observations across different binaries, not a hard upper bound. Walking calls were 4,033 / 3,951 rather than 6,155; attempts depend on path availability and crowd positions, so reduced walk-query totals alone are not evidence of equal-distance movement efficiency. An initial corner test failed, and that candidate release build was stopped before benchmarking. The failure and aborted build are retained.

The planning allowance remains cooperative: a single geometry query or short local connection can finish after the deadline. It is not advertised as a hard real-time guarantee.

## Validation and artifacts

Passed on the retained source: 39 shared navigation tests, 10 goblin tests, and two spatial partition checks (51 total). These include stacked floors, moving obstacles, corner retention, slope/step limits, resuming initial attachment, recovery backoff, and shared probe limits.

All completed runs, including exploratory runs without an improvement, are retained under `tools/build/review9-performance/`, with settings, binary hashes, raw logs, and per-window FPS/median/p95/p99 frame times. Fast-build results are not pooled with release measurements.

## Retained release candidate and follow-up

The initial release candidate (`dcbcaab484d4b759225c0b4d57ddd9777e989641df3338a59ff7ddd2838fdcad`) produced interior ABBA windows: baseline 103.4 / 102.8, candidate 103.8 / 103.4, candidate 100.6 / 105.3, baseline 92.2 / 94.7 FPS. This is a modest result with substantial baseline variation, not the large improvement suggested by the initial non-LTO cell-size experiment. All runs remain in `release-interior`.

An instrumented interior run recorded GPU median times of 1.039 ms for bin unpacking, 0.313 ms for early mesh preprocessing, and 0.786 ms for the main opaque pass. CPU render-node times do not include all ECS/render preparation. These values motivated a separate same-binary mesh-payload experiment; they do not establish a complete frame-time breakdown. Its raw log is `gpu-interior/0-final/runtime.log`.

Native baseline and initial candidate captures (`native-before`, `native-after`) show unchanged room geometry and lighting with active residents. These captures do not collect FPS. The larger payload was rejected; the retained release comparison is recorded below.

The 16,000-triangle payload experiment was rejected: at the same 8 m cell size, the 4,000-triangle control measured 117.9 / 119.0 FPS versus 109.1 / 109.1 FPS for 16,000. Candidate instances fell from 1,699 to 1,222, but candidate triangles rose from 4,002,154 to 5,105,467. The larger-payload source and binary remain in `payload`; the checkout was restored to the validated 4,000-triangle release candidate. No larger-upload change is shipped.

## Final release comparison

Each scene uses baseline / fixed / fixed / baseline order, two eight-second windows per run. The mean FPS below averages four equal-duration windows per version. Frame time is 1000 / mean FPS. No window is excluded. These are two views of one seed-42 den, not a claim covering every den or hardware configuration.

| Den view | Before FPS | After FPS | Change | Before / after mean frame time |
|---|---:|---:|---:|---:|---:|
| Interior | 98.28 | 103.28 | +5.09% | 10.176 / 9.683 ms |
| Overview | 110.85 | 110.78 | -0.07% | 9.021 / 9.027 ms |

### Raw release windows

| View | Run | Build | Window | FPS | Median ms | p95 ms | p99 ms |
|---|---:|---|---:|---:|---:|---:|---:|
| interior | 1 | before | 1 | 103.4 | 9.46 | 11.45 | 12.98 |
| interior | 1 | before | 2 | 102.8 | 9.51 | 11.90 | 13.11 |
| interior | 2 | final | 1 | 103.8 | 9.43 | 11.32 | 13.36 |
| interior | 2 | final | 2 | 103.4 | 9.45 | 11.66 | 12.88 |
| interior | 3 | final | 1 | 100.6 | 9.65 | 12.36 | 13.70 |
| interior | 3 | final | 2 | 105.3 | 9.27 | 11.63 | 13.90 |
| interior | 4 | before | 1 | 92.2 | 10.51 | 13.65 | 15.00 |
| interior | 4 | before | 2 | 94.7 | 10.21 | 13.24 | 14.56 |
| overview | 1 | before | 1 | 107.8 | 8.99 | 12.10 | 14.82 |
| overview | 1 | before | 2 | 111.6 | 8.76 | 10.69 | 11.98 |
| overview | 2 | final | 1 | 107.1 | 9.00 | 11.91 | 13.48 |
| overview | 2 | final | 2 | 110.7 | 8.83 | 10.92 | 12.28 |
| overview | 3 | final | 1 | 112.4 | 8.70 | 10.83 | 12.16 |
| overview | 3 | final | 2 | 112.9 | 8.65 | 10.75 | 12.03 |
| overview | 4 | before | 1 | 111.0 | 8.80 | 10.91 | 12.97 |
| overview | 4 | before | 2 | 113.0 | 8.70 | 10.25 | 11.90 |

The interior comparison is modest and has noticeable baseline variation (92.2–103.4 FPS). The overview difference is below meaningful run variation. Native room captures show intact surfaces/lighting and residents walking; focused tests validate supported corner movement, not every live den route. No whole-suite or strict-Clippy claim is made. All release runs exited successfully. The offscreen `ShadowLodOrigin` warning occurs in both builds; both use the same benchmark camera setup.

The tested release was installed at `target/release/hither-sdf`, SHA-256 `dcbcaab484d4b759225c0b4d57ddd9777e989641df3338a59ff7ddd2838fdcad`. Source and assets were checked byte-for-byte against the frozen `final-v2` snapshot before installation. Cargo fingerprints were not modified. Test logs are `tests-navigation-final2.log`, `tests-goblins-final2.log`, and `tests-spatial-final2.log`.

The previously installed executable had SHA-256 `c5f035a49234b2d4864f50d3bea0facfd7cdd7f2d4f0cea9d1747eb66c0b4492` and was preserved in `pre-install-binary/hither-sdf`. It is not the frozen review-8 comparator used in these tables; the exact provenance of that intervening executable was not established. Source comparison against frozen review 8 confirms that only the files listed in this review's navigation/rendering/diagnostic changes differ.
