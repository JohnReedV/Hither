# Corrected FPS measurements and frame scheduling

## Why the earlier FPS comparison was unreliable

The offscreen profiler rendered into images without a swapchain or any GPU frame-completion limit. CPU frames could therefore be counted while many frames were still waiting on the GPU. Two diagnostic runs reached 88 and 86 unfinished frames, with 55 still pending at the end of one run. The earlier 16% forest result was also based on only two runs per build and was dominated by one 104.7 FPS traversal. Those historical numbers are preserved, but are not validated measurements of sustained GPU throughput.

The offscreen profiler now waits for the older of two GPU frame submissions before proceeding. An empty submission marks all prior rendering and cache work. Diagnostic repetitions verified only one unfinished frame remained after each wait. The same correction is applied to every baseline below. Normal gameplay retains its existing swapchain presentation path. GPU timing diagnostics now also report p99 and maximum pass times.

## Game change

Bevy defaulted to 24 frame-critical compute workers on this 32-logical-CPU machine. Profiles showed significant task scheduling and mutex contention. Same-executable experiments with 1, 2, 4, 8 and default worker counts found four a better balance for this workload. The production frame pool is capped at four; separate asynchronous generation and IO defaults remain, and Bevy scales down automatically on smaller machines. No CPU affinity, process-priority changes, CPU-idle gates, resident reductions or graphics-quality cuts are applied.

The thread-count diagnostics used an additional forest location at seed 721, (0,24,3.15). The final comparisons use the original benchmark forest route at (192,24,80), with identical settings for each build. A trial GPU-buffer growth patch performed worse and was removed; the upstream Bevy 0.19.1 renderer is retained.

## Corrected optimized-release results

Original means the pre-review-5 baseline. Previous means V4, installed before this follow-up. Fixed means V4 plus the frame-worker change. All three include identical corrected profiling instrumentation. Their frozen sources/assets and SHA-256 hashes are in bounded/results/binaries.json. All use optimization level 3, fat LTO and one codegen unit.

Forest has four runs per build in alternating original/previous/fixed order and its reverse. Snow forest and the 20-resident den have two runs per build in previous/fixed/fixed/previous order. All 20 runs are retained, without concurrent builds, load-based exclusions, CPU sampling or GPU timestamp tracing. Separate diagnostic runs are not pooled here.

World rendering is 3840 x 2160, presented to a 1280 x 720 offscreen image, with FXAA, High shadows and 260/35 m render/detail distances. Forest and snow forest use seed 721, a 20-second warmup, eight seconds standing and sixteen seconds traveling at 40 m/s. The den uses seed 42, 20 residents, a 60-second warmup and two stationary eight-second windows.

Arithmetic means of per-run summaries follow. Averaged percentiles are not pooled frame percentiles. These are repeated spot checks on this machine, not a universal hardware-optimal worker-count claim.

| Scene / phase | Original FPS | Previous V4 FPS | Fixed FPS | Change vs V4 | Median ms V4 / fixed | p99 ms V4 / fixed |
|---|---:|---:|---:|---:|---:|---:|
| forest / stationary | 305.05 | 308.35 | 341.20 | +10.65% | 3.19 / 2.92 | 4.05 / 3.56 |
| forest / moving | 164.32 | 157.72 | 178.97 | +13.47% | 5.21 / 5.17 | 30.90 / 19.55 |
| boreal / stationary | — | 237.00 | 239.10 | +0.89% | 4.20 / 4.18 | 4.97 / 4.42 |
| boreal / moving | — | 157.50 | 157.80 | +0.19% | 4.71 / 4.37 | 40.65 / 42.84 |
| den-large / stationary | — | 100.15 | 123.85 | +23.66% | 9.84 / 7.92 | 13.36 / 10.75 |
| den-large / later stationary | — | 100.10 | 124.80 | +24.68% | 9.77 / 7.87 | 13.40 / 10.36 |

## Every completed final run

| Scene | Build / run | Standing FPS | Second window FPS | Runtime log |
|---|---|---:|---:|---|
| forest | original 0 | 308.1 | 168.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-0-original/forest-0/runtime.log) |
| forest | previous 1 | 305.4 | 151.9 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-1-previous/forest-0/runtime.log) |
| forest | fixed 2 | 342.5 | 189.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-2-fixed/forest-0/runtime.log) |
| forest | fixed 3 | 341.1 | 170.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-3-fixed/forest-0/runtime.log) |
| forest | previous 4 | 312.2 | 162.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-4-previous/forest-0/runtime.log) |
| forest | original 5 | 304.6 | 165.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-5-original/forest-0/runtime.log) |
| forest | fixed 6 | 340.7 | 167.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-6-fixed/forest-0/runtime.log) |
| forest | previous 7 | 309.7 | 181.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-7-previous/forest-0/runtime.log) |
| forest | original 8 | 303.8 | 168.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-8-original/forest-0/runtime.log) |
| forest | original 9 | 303.7 | 154.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-9-original/forest-0/runtime.log) |
| forest | previous 10 | 306.1 | 134.5 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-10-previous/forest-0/runtime.log) |
| forest | fixed 11 | 340.5 | 188.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/forest-11-fixed/forest-0/runtime.log) |
| boreal | previous 0 | 235.8 | 175.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/boreal-0-previous/boreal-0/runtime.log) |
| boreal | fixed 1 | 239.3 | 125.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/boreal-1-fixed/boreal-0/runtime.log) |
| boreal | fixed 2 | 238.9 | 190.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/boreal-2-fixed/boreal-0/runtime.log) |
| boreal | previous 3 | 238.2 | 139.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/boreal-3-previous/boreal-0/runtime.log) |
| den-large | previous 0 | 100.2 | 100.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/den-large-0-previous/runtime.log) |
| den-large | fixed 1 | 122.1 | 124.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/den-large-1-fixed/runtime.log) |
| den-large | fixed 2 | 125.6 | 125.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/den-large-2-fixed/runtime.log) |
| den-large | previous 3 | 100.1 | 99.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review6-performance/bounded/results/den-large-3-previous/runtime.log) |

## Validation and limitations

All optimized builds succeeded. Every final benchmark completed and its log was checked for errors/panics and the two-frame GPU limit marker. Previous den movement, collision, sky and asset fixes are preserved. Remaining occasional long traversal frames must not be described as eliminated; this pass corrects the FPS measurement and reduces measured frame scheduling overhead.

The nine completed unrestricted candidate comparisons remain under final/ with a pause/correction note. Earlier allocation experiments remain under growth/ and rejected-buffer-growth/. Diagnostic queue-depth, scheduler, GPU-time and shadow-isolation evidence remains under diagnostics/. None is silently discarded or mixed with the corrected release measurements.

The exact tested fixed executable is installed at `target/release/hither-sdf`, SHA-256 `d939ee9188214546a76d57b930876a4510c495cfdd76be0a6bc6d58a23ae8928`.
