# Performance

Measured costs for Dimora. A number here is from a named run. It is not a budget and not a claim about another machine.

## Tilemap extraction baseline

Recorded 2026-10-23 at 5:08 PM America/Chicago. The extractor is the full-grid scan at commit `c2e5150bbf0303e91ff257bd41b0378e89f97ebe`. This document's commit adds the harness and does not change extraction.

| | |
|---|---|
| Machine | Apple M5, arm64 |
| GPU | Apple M5, Metal 4. Not used by this run |
| OS | macOS 27.0 (build 26A428) |
| Rust | rustc 1.99.0 (`b940084d7` 2026-09-28) |
| Profile | `cargo run --release -p engine-renderer --bin tilemap_extract_bench` |
| Binary | `crates/engine-renderer/src/bin/tilemap_extract_bench.rs` |

`extract_tilemap` visits every cell of every map for every camera. An empty cell returns before a matrix is built and does not increment `sprites_culled`. A non-empty cell builds a world matrix, tests a bounding sphere, and either becomes one `SpriteInstance` or increments `sprites_culled`. Tiles are legacy cutout, share the map texture, and carry no material, so one camera produces one cutout batch however many tiles are visible. `cells_scanned` in the table is `width * height * cameras`, which is that loop. It is not a counter inside the extractor.

The timed region is `extract_scene` plus dropping the returned `RenderScene`. `propagate_transforms` is timed separately. Its median stayed at or under 1.1 microseconds in every scenario, so it is not the tile cost. GPU submit time was not measured. There is no GPU in this harness.

Allocation columns are one extra extract after the timed samples. They count allocator requests. A reallocation counts its new size. Frees are not subtracted. The figure is comparable across runs of this binary. It is not resident memory.

Samples, after warmup: 21 for 64² (3 warmup), 15 for 256² (2), 9 for 512² (1), 5 for 1000² (1). The median is the middle sample. p95 is nearest-rank, so on 5 samples it is the slowest sample. Times are microseconds.

The map uses 1-unit tiles, a 4×4 atlas, and one 64×64 texture. It is rotated 90 degrees about X onto the ground and centered on the origin. Sparse means `(x + y * 3)` is a multiple of 8, which is one eighth of the cells (512, 8,192, 32,768, and 125,000 occupied tiles). The viewport is 1280×720. Mostly-visible cameras sit outside the map's bounding sphere: orthographic and isometric height is 1.6 map widths, perspective is a 50 degree vertical field from 2.2 widths away. Partial cameras use height 0.28 widths, or a perspective distance of 0.38 widths, and look at a point on a ring 0.18 widths from the origin. Extra cameras are the same framing on an orbit. Isometric pitch is 0.6154797 radians with an extra yaw of 45 degrees. Near and far are 0.05 and `8 * width + 100`.

### What the baseline shows

A camera that sees the whole map pays for every occupied tile. One orthographic camera on a dense 1000×1000 map takes a median of 37.1 ms and builds one million instances in one batch. The allocator column for that row is 394,768,856 bytes. Seven such cameras take 234 ms orthographic and 295 ms perspective, build seven million instances in seven batches, and record about 2.76 GB in that same column. Those figures are cumulative allocation traffic for one `extract_scene`, not peak resident memory. The breakdown is under "What the allocation column measures". The seven-camera cost is several times one camera because each camera repeats the grid walk and owns its own instances.

A partial view still walks the whole grid. The orthographic partial camera sees about 27 percent of a dense 1000×1000 map and takes 18.0 ms, about half of the full view, not 27 percent of it. Culled tiles still pay for a matrix and a sphere test. Perspective partial sees about 49 percent and takes 21.8 ms against a 31.2 ms full view.

Empty cells are cheap relative to instance building. A fully visible sparse 1000×1000 map (125,000 occupied tiles) takes 4.3 ms orthographic with one camera, against 37.1 ms dense. The walk is still one million cells.

Batches stay at one per camera in every row. Instance count equals `visible`. Simulation time is not in the extract column.

The same binary measured the chunked extractor later the same evening. That comparison is below. Instance counts, culled counts, and batch counts matched this baseline on every row.

### 1000×1000, the acceptance size

Median milliseconds, one run. Visible fraction is the share of occupied tiles that survived culling, summed across cameras and divided by occupied tiles times cameras.

| Cameras | View | Coverage | Density | Median ms | p95 ms | Visible fraction | Instances | Batches | Alloc bytes |
|---|---|---|---|---|---|---|---|---|---|
| 1 | perspective | mostly | dense | 31.2 | 36.0 | 1.00 | 1,000,000 | 1 | 394,768,856 |
| 1 | orthographic | mostly | dense | 37.1 | 38.4 | 1.00 | 1,000,000 | 1 | 394,768,856 |
| 1 | isometric | mostly | dense | 40.5 | 43.5 | 1.00 | 1,000,000 | 1 | 394,768,856 |
| 1 | perspective | partial | dense | 21.8 | 23.2 | 0.49 | 493,588 | 1 | 196,897,992 |
| 1 | orthographic | partial | dense | 18.0 | 18.0 | 0.27 | 269,000 | 1 | 179,829,304 |
| 1 | isometric | partial | dense | 17.4 | 18.8 | 0.24 | 242,060 | 1 | 98,090,088 |
| 7 | perspective | mostly | dense | 294.6 | 363.0 | 1.00 | 7,000,000 | 7 | 2,763,375,016 |
| 7 | orthographic | mostly | dense | 234.0 | 262.9 | 1.00 | 7,000,000 | 7 | 2,763,375,016 |
| 7 | isometric | mostly | dense | 249.9 | 264.8 | 1.00 | 7,000,000 | 7 | 2,763,375,016 |
| 1 | orthographic | mostly | sparse | 4.3 | 5.2 | 1.00 | 125,000 | 1 | 49,347,640 |
| 1 | orthographic | partial | sparse | 2.2 | 2.6 | 0.27 | 33,625 | 1 | 25,035,696 |
| 7 | orthographic | mostly | sparse | 54.8 | 56.8 | 1.00 | 875,000 | 7 | 345,426,504 |
| 7 | orthographic | partial | sparse | 17.2 | 17.4 | 0.27 | 235,190 | 7 | 175,214,776 |

Times in that table are the CSV medians and p95 values rounded to one decimal millisecond. Instance counts, fractions, and allocation bytes are the CSV values. The CSV is exact.

### Full result

`visible` and `culled` sum every camera. `instances` and `batches` sum every sprite and mesh list on those passes. This scene has only cutout tile instances, so instances equal `visible` and batches equal the camera count.

```
size,cameras,view,coverage,density,populated,samples,median_us,p95_us,min_us,max_us,visible,culled,instances,batches,cells_scanned,sim_median_us,alloc_count,alloc_bytes,visible_fraction
64,1,perspective,mostly,dense,4096,21,247.5,623.2,193.2,634.2,4096,0,4096,1,4096,0.5,17,1869528,1.0000
64,1,perspective,mostly,sparse,512,21,31.8,40.2,31.4,63.3,512,0,512,1,4096,0.6,14,235224,1.0000
64,1,perspective,partial,dense,4096,21,153.4,176.1,138.7,310.6,2079,2017,2079,1,4096,0.6,17,1562944,0.5076
64,1,perspective,partial,sparse,512,21,22.4,33.8,22.0,50.8,260,252,260,1,4096,0.5,14,196920,0.5078
64,1,orthographic,mostly,dense,4096,21,188.0,255.0,185.0,279.5,4096,0,4096,1,4096,0.5,17,1869528,1.0000
64,1,orthographic,mostly,sparse,512,21,26.5,26.7,26.1,26.8,512,0,512,1,4096,0.5,14,235224,1.0000
64,1,orthographic,partial,dense,4096,21,132.4,134.4,129.2,134.6,1188,2908,1188,1,4096,0.4,16,804920,0.2900
64,1,orthographic,partial,sparse,512,21,18.8,21.5,18.5,22.6,149,363,149,1,4096,0.4,13,102224,0.2910
64,1,isometric,mostly,dense,4096,21,184.7,202.5,183.5,203.3,4096,0,4096,1,4096,0.4,17,1869528,1.0000
64,1,isometric,mostly,sparse,512,21,23.0,23.2,22.8,23.2,512,0,512,1,4096,0.4,14,235224,1.0000
64,1,isometric,partial,dense,4096,21,116.2,119.4,114.5,124.0,1140,2956,1140,1,4096,0.3,16,797624,0.2783
64,1,isometric,partial,sparse,512,21,17.5,18.4,17.2,18.7,143,369,143,1,4096,0.4,13,101312,0.2793
64,2,perspective,mostly,dense,4096,21,295.2,351.1,289.1,372.7,8192,0,8192,2,8192,0.4,30,3737488,1.0000
64,2,perspective,mostly,sparse,512,21,39.6,44.5,39.4,45.0,1024,0,1024,2,8192,0.4,24,468880,1.0000
64,2,perspective,partial,dense,4096,21,205.2,242.8,202.0,302.1,4158,4034,4158,2,8192,0.3,30,3124320,0.5076
64,2,perspective,partial,sparse,512,21,30.9,31.5,30.5,31.8,520,504,520,2,8192,0.3,24,392272,0.5078
64,2,orthographic,mostly,dense,4096,21,251.5,257.8,231.2,262.5,8192,0,8192,2,8192,0.3,30,3737488,1.0000
64,2,orthographic,mostly,sparse,512,21,32.7,32.9,32.4,33.3,1024,0,1024,2,8192,0.2,24,468880,1.0000
64,2,orthographic,partial,dense,4096,21,152.7,165.0,151.4,170.4,2376,5816,2376,2,8192,0.2,28,1608272,0.2900
64,2,orthographic,partial,sparse,512,21,21.5,21.6,21.3,21.9,297,727,297,2,8192,0.2,22,202728,0.2900
64,2,isometric,mostly,dense,4096,21,202.9,206.4,202.1,208.1,8192,0,8192,2,8192,0.2,30,3737488,1.0000
64,2,isometric,mostly,sparse,512,21,28.6,34.0,28.2,39.8,1024,0,1024,2,8192,0.2,24,468880,1.0000
64,2,isometric,partial,dense,4096,21,143.8,173.3,136.4,173.3,2280,5912,2280,2,8192,0.2,28,1593680,0.2783
64,2,isometric,partial,sparse,512,21,20.8,21.8,20.5,22.4,286,738,286,2,8192,0.2,22,201056,0.2793
64,7,perspective,mostly,dense,4096,21,624.9,638.2,580.5,724.1,28672,0,28672,7,28672,0.3,96,13079720,1.0000
64,7,perspective,mostly,sparse,512,21,79.7,85.3,79.0,88.5,3584,0,3584,7,28672,0.3,75,1639592,1.0000
64,7,perspective,partial,dense,4096,21,469.3,503.0,463.4,531.3,15003,13669,15003,7,28672,0.3,93,9134256,0.5233
64,7,perspective,partial,sparse,512,21,72.8,75.9,72.4,75.9,1883,1701,1883,7,28672,0.3,72,1147568,0.5254
64,7,orthographic,mostly,dense,4096,21,581.2,591.9,573.5,599.0,28672,0,28672,7,28672,0.3,96,13079720,1.0000
64,7,orthographic,mostly,sparse,512,21,79.2,79.8,78.5,79.8,3584,0,3584,7,28672,0.3,75,1639592,1.0000
64,7,orthographic,partial,dense,4096,21,417.5,450.8,408.1,471.7,8562,20110,8562,7,28672,0.3,89,5664856,0.2986
64,7,orthographic,partial,sparse,512,21,63.1,66.2,62.4,66.3,1070,2514,1070,7,28672,0.3,68,712696,0.2985
64,7,isometric,mostly,dense,4096,21,580.5,586.0,577.0,598.5,28672,0,28672,7,28672,0.3,96,13079720,1.0000
64,7,isometric,mostly,sparse,512,21,79.2,82.3,78.4,82.5,3584,0,3584,7,28672,0.3,75,1639592,1.0000
64,7,isometric,partial,dense,4096,21,406.0,417.7,394.6,426.0,7811,20861,7811,7,28672,0.3,89,5550704,0.2724
64,7,isometric,partial,sparse,512,21,61.5,65.1,60.8,66.3,971,2613,971,7,28672,0.2,68,697648,0.2709
256,1,perspective,mostly,dense,65536,15,2134.9,2925.4,1818.6,2925.4,65536,0,65536,1,65536,0.2,21,27924608,1.0000
256,1,perspective,mostly,sparse,8192,15,170.7,179.4,167.2,179.4,8192,0,8192,1,65536,0.2,18,3737304,1.0000
256,1,perspective,partial,dense,65536,15,1390.9,1596.9,1263.3,1596.9,32540,32996,32540,1,65536,0.2,20,14909304,0.4965
256,1,perspective,partial,sparse,8192,15,133.5,153.0,133.0,153.0,4068,4124,4068,1,65536,0.2,17,1865272,0.4966
256,1,orthographic,mostly,dense,65536,15,2144.8,2691.5,1842.0,2691.5,65536,0,65536,1,65536,0.2,21,27924608,1.0000
256,1,orthographic,mostly,sparse,8192,15,165.7,168.9,162.4,168.9,8192,0,8192,1,65536,0.2,18,3737304,1.0000
256,1,orthographic,partial,dense,65536,15,1187.8,1410.7,1032.8,1410.7,18060,47476,18060,1,65536,0.2,20,12708344,0.2756
256,1,orthographic,partial,sparse,8192,15,119.9,136.5,119.1,136.5,2257,5935,2257,1,65536,0.2,17,1590000,0.2755
256,1,isometric,mostly,dense,65536,15,2037.5,2291.8,1942.5,2291.8,65536,0,65536,1,65536,0.2,21,27924608,1.0000
256,1,isometric,mostly,sparse,8192,15,164.1,170.4,163.5,170.4,8192,0,8192,1,65536,0.2,18,3737304,1.0000
256,1,isometric,partial,dense,65536,15,892.1,948.5,876.0,948.5,16306,49230,16306,1,65536,0.2,19,7461000,0.2488
256,1,isometric,partial,sparse,8192,15,117.8,136.0,117.0,136.0,2060,6132,2060,1,65536,0.2,17,1560056,0.2515
256,2,perspective,mostly,dense,65536,15,4448.4,5521.2,3867.8,5521.2,131072,0,131072,2,131072,0.2,38,55847648,1.0000
256,2,perspective,mostly,sparse,8192,15,330.1,364.8,328.5,364.8,16384,0,16384,2,131072,0.2,32,7473040,1.0000
256,2,perspective,partial,dense,65536,15,3018.0,3774.8,2799.5,3774.8,65080,65992,65080,2,131072,0.2,36,29817040,0.4965
256,2,perspective,partial,sparse,8192,15,268.8,279.5,264.4,279.5,8134,8250,8134,2,131072,0.2,30,3728672,0.4965
256,2,orthographic,mostly,dense,65536,15,4317.6,5165.5,4181.3,5165.5,131072,0,131072,2,131072,0.2,38,55847648,1.0000
256,2,orthographic,mostly,sparse,8192,15,335.9,348.7,328.6,348.7,16384,0,16384,2,131072,0.2,32,7473040,1.0000
256,2,orthographic,partial,dense,65536,15,2243.1,2621.7,2173.5,2621.7,36120,94952,36120,2,131072,0.2,36,25415120,0.2756
256,2,orthographic,partial,sparse,8192,15,236.5,257.8,233.6,257.8,4515,11869,4515,2,131072,0.2,30,3178584,0.2756
256,2,isometric,mostly,dense,65536,15,4310.7,4425.4,3942.5,4425.4,131072,0,131072,2,131072,0.2,38,55847648,1.0000
256,2,isometric,mostly,sparse,8192,15,334.1,366.0,326.0,366.0,16384,0,16384,2,131072,0.2,32,7473040,1.0000
256,2,isometric,partial,dense,65536,15,1795.8,1996.3,1761.6,1996.3,32612,98460,32612,2,131072,0.2,34,14920432,0.2488
256,2,isometric,partial,sparse,8192,15,236.2,249.3,234.1,249.3,4120,12264,4120,2,131072,0.2,30,3118544,0.2515
256,7,perspective,mostly,dense,65536,15,15088.0,16791.9,14513.8,16791.9,458752,0,458752,7,458752,0.2,124,195465280,1.0000
256,7,perspective,mostly,sparse,8192,15,1184.6,1240.7,1160.7,1240.7,57344,0,57344,7,458752,0.3,103,26154152,1.0000
256,7,perspective,partial,dense,65536,15,10627.5,11677.1,9962.6,11677.1,233762,224990,233762,7,458752,0.3,120,135151832,0.5096
256,7,perspective,partial,sparse,8192,15,968.0,1047.1,955.9,1047.1,29228,28116,29228,7,458752,0.3,99,16899784,0.5097
256,7,orthographic,mostly,dense,65536,15,14708.8,16348.7,13969.3,16348.7,458752,0,458752,7,458752,0.3,124,195465280,1.0000
256,7,orthographic,mostly,sparse,8192,15,1175.4,1198.5,1158.6,1198.5,57344,0,57344,7,458752,0.4,103,26154152,1.0000
256,7,orthographic,partial,dense,65536,15,8377.9,9488.9,7654.8,9488.9,126144,332608,126144,7,458752,0.3,117,88909480,0.2750
256,7,orthographic,partial,sparse,8192,15,840.0,871.6,836.0,871.6,15768,41576,15768,7,458752,0.3,96,11118312,0.2750
256,7,isometric,mostly,dense,65536,15,14534.0,15275.2,13213.1,15275.2,458752,0,458752,7,458752,0.3,124,195465280,1.0000
256,7,isometric,mostly,sparse,8192,15,1158.8,1182.6,1153.5,1182.6,57344,0,57344,7,458752,0.3,103,26154152,1.0000
256,7,isometric,partial,dense,65536,15,6285.8,6723.5,6206.5,6723.5,114124,344628,114124,7,458752,0.3,110,52217288,0.2488
256,7,isometric,partial,sparse,8192,15,827.4,846.8,819.8,846.8,14287,43057,14287,7,458752,0.3,90,7157648,0.2491
512,1,perspective,mostly,dense,262144,9,10141.7,10954.2,7736.3,10954.2,262144,0,262144,1,262144,0.2,23,99616472,1.0000
512,1,perspective,mostly,sparse,32768,9,1033.9,1147.9,964.8,1147.9,32768,0,32768,1,262144,0.2,20,14943960,1.0000
512,1,perspective,partial,dense,262144,9,5937.7,9343.4,5527.7,9343.4,129755,132389,129755,1,262144,0.2,22,49709096,0.4950
512,1,perspective,partial,sparse,32768,9,537.4,601.6,528.1,601.6,16220,16548,16220,1,262144,0.2,19,7447928,0.4950
512,1,orthographic,mostly,dense,262144,9,11089.4,12102.4,10272.4,12102.4,262144,0,262144,1,262144,0.2,23,99616472,1.0000
512,1,orthographic,mostly,sparse,32768,9,1176.5,1288.3,1129.0,1288.3,32768,0,32768,1,262144,0.2,20,14943960,1.0000
512,1,orthographic,partial,dense,262144,9,5116.0,7272.5,4874.2,7272.5,70656,191488,70656,1,262144,0.2,22,47847552,0.2695
512,1,orthographic,partial,sparse,32768,9,474.8,478.6,470.3,478.6,8832,23936,8832,1,262144,0.2,19,6324952,0.2695
512,1,isometric,mostly,dense,262144,9,8426.4,12931.8,7788.8,12931.8,262144,0,262144,1,262144,0.2,23,99616472,1.0000
512,1,isometric,mostly,sparse,32768,9,1400.1,2037.1,1023.6,2037.1,32768,0,32768,1,262144,0.2,20,14943960,1.0000
512,1,isometric,partial,dense,262144,9,4585.7,4829.1,4143.6,4829.1,64192,197952,64192,1,262144,0.2,21,27924608,0.2449
512,1,isometric,partial,sparse,32768,9,445.4,459.6,443.9,459.6,8067,24701,8067,1,262144,0.2,18,3718304,0.2462
512,2,perspective,mostly,dense,262144,9,20350.8,26303.5,15433.5,26303.5,524288,0,524288,2,524288,0.2,42,199231376,1.0000
512,2,perspective,mostly,sparse,32768,9,2378.3,5841.2,1759.8,5841.2,65536,0,65536,2,524288,0.2,36,29886352,1.0000
512,2,perspective,partial,dense,262144,9,11504.8,12295.5,11003.5,12295.5,259510,264778,259510,2,524288,0.2,40,99416624,0.4950
512,2,perspective,partial,sparse,32768,9,1061.5,1115.3,1055.5,1115.3,32439,33097,32439,2,524288,0.2,34,14894136,0.4950
512,2,orthographic,mostly,dense,262144,9,18692.0,19299.8,16472.9,19299.8,524288,0,524288,2,524288,0.2,42,199231376,1.0000
512,2,orthographic,mostly,sparse,32768,9,1987.2,3458.1,1916.2,3458.1,65536,0,65536,2,524288,0.2,36,29886352,1.0000
512,2,orthographic,partial,dense,262144,9,9261.8,10036.4,8948.5,10036.4,141312,382976,141312,2,524288,0.2,40,95693536,0.2695
512,2,orthographic,partial,sparse,32768,9,939.2,972.8,923.8,972.8,17664,47872,17664,2,524288,0.2,34,12648336,0.2695
512,2,isometric,mostly,dense,262144,9,17594.2,18021.5,17034.2,18021.5,524288,0,524288,2,524288,0.2,42,199231376,1.0000
512,2,isometric,mostly,sparse,32768,9,2023.7,2082.2,1904.9,2082.2,65536,0,65536,2,524288,0.3,36,29886352,1.0000
512,2,isometric,partial,dense,262144,9,8691.5,8927.0,8503.2,8927.0,128384,395904,128384,2,524288,0.2,38,55847648,0.2449
512,2,isometric,partial,sparse,32768,9,905.8,921.9,883.2,921.9,16134,49402,16134,2,524288,0.2,32,7435040,0.2462
512,7,perspective,mostly,dense,262144,9,74168.5,91015.9,67634.4,91015.9,1835008,0,1835008,7,1835008,0.3,138,697308328,1.0000
512,7,perspective,mostly,sparse,32768,9,9506.9,9962.6,6645.9,9962.6,229376,0,229376,7,1835008,0.4,117,104600744,1.0000
512,7,perspective,partial,dense,262144,9,38343.8,40575.1,37679.0,40575.1,930785,904223,930785,7,1835008,0.3,134,470933208,0.5072
512,7,perspective,partial,sparse,32768,9,4849.1,4975.9,4743.1,4975.9,116346,113030,116346,7,1835008,0.4,113,67497240,0.5072
512,7,orthographic,mostly,dense,262144,9,60176.6,73455.1,56386.6,73455.1,1835008,0,1835008,7,1835008,0.3,138,697308328,1.0000
512,7,orthographic,mostly,sparse,32768,9,10405.8,13949.5,8103.2,13949.5,229376,0,229376,7,1835008,0.3,117,104600744,1.0000
512,7,orthographic,partial,dense,262144,9,31344.7,31743.1,30626.2,31743.1,496516,1338492,496516,7,1835008,0.3,131,334925888,0.2706
512,7,orthographic,partial,sparse,32768,9,3335.5,3441.8,3274.8,3441.8,62061,167315,62061,7,1835008,0.4,110,44303712,0.2706
512,7,isometric,mostly,dense,262144,9,61956.6,72193.4,56649.6,72193.4,1835008,0,1835008,7,1835008,0.3,138,697308328,1.0000
512,7,isometric,mostly,sparse,32768,9,10413.9,14518.2,6720.9,14518.2,229376,0,229376,7,1835008,0.3,117,104600744,1.0000
512,7,isometric,partial,dense,262144,9,29219.6,30421.3,28618.0,30421.3,449604,1385404,449604,7,1835008,0.3,124,195465280,0.2450
512,7,isometric,partial,sparse,32768,9,3209.9,3316.7,3191.5,3316.7,56237,173139,56237,7,1835008,0.3,103,25985888,0.2452
1000,1,perspective,mostly,dense,1000000,5,31249.4,36045.2,30472.5,36045.2,1000000,0,1000000,1,1000000,0.2,25,394768856,1.0000
1000,1,perspective,mostly,sparse,125000,5,3856.5,4025.3,3488.3,4025.3,125000,0,125000,1,1000000,0.2,22,49347640,1.0000
1000,1,perspective,partial,dense,1000000,5,21795.3,23174.6,21376.8,23174.6,493588,506412,493588,1,1000000,0.2,24,196897992,0.4936
1000,1,perspective,partial,sparse,125000,5,2836.3,2924.8,2735.1,2924.8,61699,63301,61699,1,1000000,0.2,21,27924608,0.4936
1000,1,orthographic,mostly,dense,1000000,5,37094.1,38363.8,36917.0,38363.8,1000000,0,1000000,1,1000000,0.2,25,394768856,1.0000
1000,1,orthographic,mostly,sparse,125000,5,4268.2,5239.1,4039.0,5239.1,125000,0,125000,1,1000000,0.2,22,49347640,1.0000
1000,1,orthographic,partial,dense,1000000,5,17950.8,17978.7,17241.9,17978.7,269000,731000,269000,1,1000000,0.2,24,179829304,0.2690
1000,1,orthographic,partial,sparse,125000,5,2241.2,2647.6,2125.0,2647.6,33625,91375,33625,1,1000000,0.2,21,25035696,0.2690
1000,1,isometric,mostly,dense,1000000,5,40458.0,43485.5,39849.7,43485.5,1000000,0,1000000,1,1000000,0.2,25,394768856,1.0000
1000,1,isometric,mostly,sparse,125000,5,8806.3,9429.7,4069.0,9429.7,125000,0,125000,1,1000000,0.2,22,49347640,1.0000
1000,1,isometric,partial,dense,1000000,5,17397.6,18795.4,16370.6,18795.4,242060,757940,242060,1,1000000,0.2,23,98090088,0.2421
1000,1,isometric,partial,sparse,125000,5,4128.5,4541.1,2268.5,4541.1,30339,94661,30339,1,1000000,0.2,20,14574752,0.2427
1000,2,perspective,mostly,dense,1000000,5,74514.8,87073.9,71255.6,87073.9,2000000,0,2000000,2,2000000,0.2,46,789536144,1.0000
1000,2,perspective,mostly,sparse,125000,5,10022.6,10933.7,9313.8,10933.7,250000,0,250000,2,2000000,0.2,40,98693712,1.0000
1000,2,perspective,partial,dense,1000000,5,47207.2,52407.5,44834.2,52407.5,987176,1012824,987176,2,2000000,0.2,44,393794416,0.4936
1000,2,perspective,partial,sparse,125000,5,6258.1,7234.2,5876.8,7234.2,123397,126603,123397,2,2000000,0.3,38,55847648,0.4936
1000,2,orthographic,mostly,dense,1000000,5,70847.5,98861.3,63978.1,98861.3,2000000,0,2000000,2,2000000,0.2,46,789536144,1.0000
1000,2,orthographic,mostly,sparse,125000,5,8573.1,10348.5,7782.2,10348.5,250000,0,250000,2,2000000,0.2,40,98693712,1.0000
1000,2,orthographic,partial,dense,1000000,5,35077.3,39470.5,33431.9,39470.5,538000,1462000,538000,2,2000000,0.2,44,359657040,0.2690
1000,2,orthographic,partial,sparse,125000,5,4715.5,5105.1,4657.4,5105.1,67250,182750,67250,2,2000000,0.2,38,50069824,0.2690
1000,2,isometric,mostly,dense,1000000,5,83487.2,85745.4,77295.5,85745.4,2000000,0,2000000,2,2000000,0.2,46,789536144,1.0000
1000,2,isometric,mostly,sparse,125000,5,9044.2,13095.8,8429.6,13095.8,250000,0,250000,2,2000000,0.2,40,98693712,1.0000
1000,2,isometric,partial,dense,1000000,5,33934.1,34266.0,33199.8,34266.0,484120,1515880,484120,2,2000000,0.2,42,196178608,0.2421
1000,2,isometric,partial,sparse,125000,5,4684.1,6045.8,4502.5,6045.8,60678,189322,60678,2,2000000,0.2,36,29147936,0.2427
1000,7,perspective,mostly,dense,1000000,5,294612.3,362999.0,257412.8,362999.0,7000000,0,7000000,7,7000000,0.3,152,2763375016,1.0000
1000,7,perspective,mostly,sparse,125000,5,49189.1,54819.3,39025.2,54819.3,875000,0,875000,7,7000000,1.1,131,345426504,1.0000
1000,7,perspective,partial,dense,1000000,5,155665.2,163159.8,151313.3,163159.8,3541993,3458007,3541993,7,7000000,0.3,148,1863032352,0.5060
1000,7,perspective,partial,sparse,125000,5,20122.7,20836.3,18631.8,20836.3,442743,432257,442743,7,7000000,0.4,127,252672152,0.5060
1000,7,orthographic,mostly,dense,1000000,5,234022.3,262873.8,221022.1,262873.8,7000000,0,7000000,7,7000000,0.3,152,2763375016,1.0000
1000,7,orthographic,mostly,sparse,125000,5,54793.8,56819.6,35231.8,56819.6,875000,0,875000,7,7000000,0.3,131,345426504,1.0000
1000,7,orthographic,partial,dense,1000000,5,121737.6,128393.8,120827.4,128393.8,1881521,5118479,1881521,7,7000000,0.3,145,1258685976,0.2688
1000,7,orthographic,partial,sparse,125000,5,17195.5,17372.8,16351.4,17372.8,235190,639810,235190,7,7000000,0.3,124,175214776,0.2688
1000,7,isometric,mostly,dense,1000000,5,249859.9,264797.3,224242.4,264797.3,7000000,0,7000000,7,7000000,0.3,152,2763375016,1.0000
1000,7,isometric,mostly,sparse,125000,5,50403.2,53668.9,35078.0,53668.9,875000,0,875000,7,7000000,0.4,131,345426504,1.0000
1000,7,isometric,partial,dense,1000000,5,113044.0,121575.7,112459.0,121575.7,1701429,5298571,1701429,7,7000000,0.3,138,687156552,0.2431
1000,7,isometric,partial,sparse,125000,5,14996.5,15351.0,14829.2,15351.0,212757,662243,212757,7,7000000,0.3,117,102074656,0.2432
```

## Tilemap chunks, 32×32

Recorded 2026-10-23 at 5:26 PM America/Chicago on the same machine, profile, and harness as the baseline. `TileMap::CHUNK_EDGE` is 32. The run rebuilt `engine-renderer` with that default. `chunks_rebuilt` is 0 in the timed samples because warmup already filled the occupancy index. `tiles_visited` counts cells in accepted chunks, including empty cells. `cells_scanned` is still the old full-grid product, kept so the two columns can be compared.

16, 32, and 64 were each measured with the full scenario list. 16 rejected a few more fringe tiles on partial views and was the fastest partial case in a handful of rows. It was also slower when a large map was fully visible to several cameras: a dense 1000×1000 orthographic view with seven cameras went from 234 ms to 281 ms, about 20 percent. 64 was close to 32 on full views and a little slower on partial views (a dense 1000×1000 orthographic partial view was 11.2 ms against 10.2 ms). 32 did not make any dense, mostly-visible scenario more than 5 percent slower than the baseline. The seven-camera orthographic full view was 236 ms against 234 ms. That is the default.

A camera that sees every chunk still reads every cell. The win is rejecting a chunk before that read. Occupancy of a rejected chunk is a stored count, refreshed only when `set` or `invalidate` marks the chunk dirty. Sprite instances are not cached. Each visible tile still builds a world matrix, because the map transform can change every frame. Owner revision 2026-10-26 (M6 Option A): this occupancy invalidation and frustum cull are the accepted M6 tilemap contract. The 2026-10-13 request for a retained per-chunk instance cache is deferred to KI-31. The full-view numbers below remain the measurement of rebuilding visible instances. They are not a claim that the cache exists.

### 1000×1000 after 32×32 chunks

Median milliseconds. A positive change is faster. Instance and batch counts are unchanged from the baseline, so they are not repeated.

| Cameras | View | Coverage | Density | Before | After | Change | Tiles visited | Chunks tested | Chunks rejected |
|---|---|---|---|---|---|---|---|---|---|
| 1 | perspective | mostly | dense | 31.2 | 30.0 | 4% | 1,000,000 | 1,024 | 0 |
| 1 | orthographic | mostly | dense | 37.1 | 30.5 | 18% | 1,000,000 | 1,024 | 0 |
| 1 | isometric | mostly | dense | 40.5 | 36.6 | 10% | 1,000,000 | 1,024 | 0 |
| 1 | perspective | partial | dense | 21.8 | 15.8 | 27% | 528,960 | 1,024 | 472 |
| 1 | orthographic | partial | dense | 18.0 | 10.2 | 43% | 330,752 | 1,024 | 701 |
| 1 | isometric | partial | dense | 17.4 | 8.8 | 50% | 302,080 | 1,024 | 726 |
| 7 | perspective | mostly | dense | 294.6 | 259.3 | 12% | 7,000,000 | 7,168 | 0 |
| 7 | orthographic | mostly | dense | 234.0 | 236.4 | -1% | 7,000,000 | 7,168 | 0 |
| 7 | isometric | mostly | dense | 249.9 | 226.8 | 9% | 7,000,000 | 7,168 | 0 |
| 7 | orthographic | partial | dense | 121.7 | 68.4 | 44% | 2,347,264 | 7,168 | 4,872 |
| 1 | orthographic | mostly | sparse | 4.3 | 4.2 | 1% | 1,000,000 | 1,024 | 0 |
| 1 | orthographic | partial | sparse | 2.2 | 1.2 | 46% | 330,752 | 1,024 | 701 |
| 7 | orthographic | partial | sparse | 17.2 | 10.3 | 40% | 2,347,264 | 7,168 | 4,872 |

The sparse pattern fills every 32×32 square, so a fully visible sparse map still walks every cell. Its one-camera time is unchanged. Partial sparse maps skip the rejected squares. The one-camera dense full view records 394,769,896 allocation bytes after chunking, 1,040 more than the baseline. Seven dense full views record 2,763,382,296. Batches stay at one per camera.

### What the allocation column measures

`alloc_bytes` is the sum of `Layout::size` for every `alloc` and `realloc` during one `extract_scene` after the timed samples. A reallocation counts the new size and does not subtract the freed buffer. The counter is reset immediately before that extract, so storage that already exists is not included. The column is not peak resident memory, not retained capacity, and not a GPU allocation. The harness has no GPU.

`SpriteInstance` is 152 bytes, aligned to 8 (`a_thousand_square_occupancy_index_is_eight_kibibytes_at_the_default_edge`). For one million instances Rust 1.99 grows the vector through capacities 4, 8, …, 1,048,576. The sum of those capacities is 2,097,148 elements, which is 318,766,496 bytes of traffic. The vector that survives the extract holds a capacity of 1,048,576 elements: 159,383,552 bytes live, of which 152,000,000 are the million instances and the rest is spare capacity. The earlier, smaller buffers have been freed.

`cutout.sort_by_key` then runs driftsort. For this length the scratch buffer is `len - len / 2` = 500,000 instances, because `8_000_000 / 152` is smaller than that half. That is one allocation of 76,000,000 bytes, freed when the sort returns. During the sort the live set is the finished instance vector plus that scratch, about 235 MB, and then the scratch is gone. The returned `RenderScene` keeps the instance vector.

Those two requests are 394,766,496 bytes. The recorded one-camera row is 394,769,896, so about 3.4 KB is everything else in that extract: the pass list, the prepared tile, the batch list, and, after chunking, the 1,024-byte chunk-visibility bit vector. Seven cameras repeat the instance vector and the sort scratch, 7 × 394,766,496 = 2,763,365,472 bytes, against a recorded 2,763,382,296. The gap is about 17 KB of the same small requests. At the end of the seven-camera extract the scene is holding seven instance vectors at once, 1,064 MiB of capacity, until the scene is dropped. The 2.76 GB figure is the traffic that built them, including buffers that have already been freed. It is not 2.76 GB resident.

Not in that column, because they are allocated earlier or on another device:

| Source | Where it lives | 1000×1000 dense, one full camera |
|---|---|---|
| Tile storage | `TileMap.tiles`, allocated with the world, before the counter starts | 2,000,000 bytes |
| Chunk occupancy | `ChunkRecord` is 8 bytes (`bool` plus `u32`). 32×32 chunks of a 1000×1000 map is 1,024 records. Built on the first extract, so warmup has already paid for it | 8,192 bytes |
| GPU instance upload | Not run by this binary. `GpuSpriteInstance` is 112 bytes. The backend copies instances into a buffer that starts at 4,096 bytes, grows to the next power of two of the upload, and does not shrink | Not measured |

A fully visible million-tile camera therefore spends most of its allocation traffic on short-lived instance storage, and a seven-camera frame keeps 1,064 MiB of that capacity until the scene drops. That cost is unchanged by chunk culling, because a full view still emits every tile. It is recorded as follow-up KI-31. This checkpoint does not add a retained tile world to avoid it.

Limits of this measurement: GPU time is not included. The sphere test is conservative, so a partial view visits some tiles the per-tile test then culls (330,752 cells visited for 269,000 drawn on the orthographic partial map). A direct write to `tiles` does not hide a tile from a camera that can see it; the next extract reads the cell. The rejected chunk's culled-tile count stays stale until `invalidate`. `width` and `height` changes rebuild the index without `invalidate`. Tiles have no material asset. `CoplanarPriority` on the entity is ignored. Future tile painting should call `set` for one cell, or `invalidate` after replacing `tiles`. Future collision can reuse `CHUNK_EDGE` and should not add a second extractor. Those systems are not built here. The full-view instance traffic above is the remaining extraction-cost risk.

GPU submit time was not measured. The GPU tests and the release screenshots check the picture, not the frame time.

### Full result after 32×32 chunks

```
size,cameras,view,coverage,density,populated,samples,median_us,p95_us,min_us,max_us,visible,culled,instances,batches,cells_scanned,sim_median_us,alloc_count,alloc_bytes,visible_fraction,tiles_visited,chunks_tested,chunks_rejected,chunks_rebuilt
64,1,perspective,mostly,dense,4096,21,189.8,249.4,188.0,261.9,4096,0,4096,1,4096,0.6,18,1869548,1.0000,4096,4,0,0
64,1,perspective,mostly,sparse,512,21,23.6,24.0,23.4,31.0,512,0,512,1,4096,0.5,15,235244,1.0000,4096,4,0,0
64,1,perspective,partial,dense,4096,21,148.1,149.3,127.7,151.0,2079,2017,2079,1,4096,0.4,18,1562964,0.5076,4096,4,0,0
64,1,perspective,partial,sparse,512,21,23.1,23.8,22.9,24.4,260,252,260,1,4096,0.4,15,196940,0.5078,4096,4,0,0
64,1,orthographic,mostly,dense,4096,21,186.0,209.6,161.1,211.6,4096,0,4096,1,4096,0.4,18,1869548,1.0000,4096,4,0,0
64,1,orthographic,mostly,sparse,512,21,23.7,23.9,23.5,24.0,512,0,512,1,4096,0.3,15,235244,1.0000,4096,4,0,0
64,1,orthographic,partial,dense,4096,21,114.4,116.4,113.8,118.2,1188,2908,1188,1,4096,0.4,17,804940,0.2900,4096,4,0,0
64,1,orthographic,partial,sparse,512,21,17.0,17.1,16.7,27.5,149,363,149,1,4096,0.3,14,102244,0.2910,4096,4,0,0
64,1,isometric,mostly,dense,4096,21,161.2,162.9,142.0,168.5,4096,0,4096,1,4096,0.3,18,1869548,1.0000,4096,4,0,0
64,1,isometric,mostly,sparse,512,21,20.8,21.0,20.7,21.1,512,0,512,1,4096,0.3,15,235244,1.0000,4096,4,0,0
64,1,isometric,partial,dense,4096,21,101.0,102.3,99.9,103.5,1140,2956,1140,1,4096,0.3,17,797644,0.2783,4096,4,0,0
64,1,isometric,partial,sparse,512,21,15.6,16.0,15.5,16.7,143,369,143,1,4096,0.3,14,101332,0.2793,4096,4,0,0
64,2,perspective,mostly,dense,4096,21,255.7,284.4,252.8,286.6,8192,0,8192,2,8192,0.3,32,3737528,1.0000,8192,8,0,0
64,2,perspective,mostly,sparse,512,21,36.3,36.9,36.0,37.2,1024,0,1024,2,8192,0.3,26,468920,1.0000,8192,8,0,0
64,2,perspective,partial,dense,4096,21,200.9,202.1,183.9,209.0,4158,4034,4158,2,8192,0.3,32,3124360,0.5076,8192,8,0,0
64,2,perspective,partial,sparse,512,21,29.2,29.5,28.9,29.9,520,504,520,2,8192,0.3,26,392312,0.5078,8192,8,0,0
64,2,orthographic,mostly,dense,4096,21,216.5,233.3,215.1,235.6,8192,0,8192,2,8192,0.3,32,3737528,1.0000,8192,8,0,0
64,2,orthographic,mostly,sparse,512,21,31.0,31.3,30.8,31.3,1024,0,1024,2,8192,0.3,26,468920,1.0000,8192,8,0,0
64,2,orthographic,partial,dense,4096,21,152.9,168.6,142.1,177.0,2376,5816,2376,2,8192,0.2,30,1608312,0.2900,8192,8,0,0
64,2,orthographic,partial,sparse,512,21,20.8,21.0,20.5,21.1,297,727,297,2,8192,0.2,24,202768,0.2900,8192,8,0,0
64,2,isometric,mostly,dense,4096,21,203.0,203.8,191.2,205.5,8192,0,8192,2,8192,0.2,32,3737528,1.0000,8192,8,0,0
64,2,isometric,mostly,sparse,512,21,27.5,27.8,27.3,27.9,1024,0,1024,2,8192,0.2,26,468920,1.0000,8192,8,0,0
64,2,isometric,partial,dense,4096,21,129.6,135.3,128.9,136.0,2280,5912,2280,2,8192,0.2,30,1593720,0.2783,8192,8,0,0
64,2,isometric,partial,sparse,512,21,19.9,20.3,19.8,21.2,286,738,286,2,8192,0.2,24,201096,0.2793,8192,8,0,0
64,7,perspective,mostly,dense,4096,21,581.3,626.7,577.6,626.9,28672,0,28672,7,28672,0.3,103,13079860,1.0000,28672,28,0,0
64,7,perspective,mostly,sparse,512,21,81.1,81.7,80.7,89.3,3584,0,3584,7,28672,0.3,82,1639732,1.0000,28672,28,0,0
64,7,perspective,partial,dense,4096,21,462.9,469.6,460.1,470.0,15003,13669,15003,7,28672,0.4,100,9134396,0.5233,28672,28,0,0
64,7,perspective,partial,sparse,512,21,74.8,78.0,74.1,78.1,1883,1701,1883,7,28672,0.3,79,1147708,0.5254,28672,28,0,0
64,7,orthographic,mostly,dense,4096,21,579.3,590.9,575.8,599.4,28672,0,28672,7,28672,0.3,103,13079860,1.0000,28672,28,0,0
64,7,orthographic,mostly,sparse,512,21,80.5,84.6,77.3,90.0,3584,0,3584,7,28672,0.3,82,1639732,1.0000,28672,28,0,0
64,7,orthographic,partial,dense,4096,21,413.5,419.6,405.6,434.5,8562,20110,8562,7,28672,0.3,96,5664996,0.2986,28672,28,0,0
64,7,orthographic,partial,sparse,512,21,65.0,67.4,64.7,69.8,1070,2514,1070,7,28672,0.3,75,712836,0.2985,28672,28,0,0
64,7,isometric,mostly,dense,4096,21,575.9,596.3,569.7,605.4,28672,0,28672,7,28672,0.3,103,13079860,1.0000,28672,28,0,0
64,7,isometric,mostly,sparse,512,21,80.3,83.2,80.0,83.7,3584,0,3584,7,28672,0.4,82,1639732,1.0000,28672,28,0,0
64,7,isometric,partial,dense,4096,21,402.0,420.4,398.7,434.0,7811,20861,7811,7,28672,0.3,96,5550844,0.2724,28672,28,0,0
64,7,isometric,partial,sparse,512,21,64.3,70.7,64.0,70.7,971,2613,971,7,28672,0.3,75,697788,0.2709,28672,28,0,0
256,1,perspective,mostly,dense,65536,15,1750.2,1858.2,1670.5,1858.2,65536,0,65536,1,65536,0.2,22,27924688,1.0000,65536,64,0,0
256,1,perspective,mostly,sparse,8192,15,169.0,172.8,168.4,172.8,8192,0,8192,1,65536,0.2,19,3737384,1.0000,65536,64,0,0
256,1,perspective,partial,dense,65536,15,963.4,999.2,936.6,999.2,32540,32996,32540,1,65536,0.2,21,14909384,0.4965,40960,64,24,0
256,1,perspective,partial,sparse,8192,15,100.3,107.2,100.1,107.2,4068,4124,4068,1,65536,0.2,18,1865352,0.4966,40960,64,24,0
256,1,orthographic,mostly,dense,65536,15,1776.1,1866.2,1690.7,1866.2,65536,0,65536,1,65536,0.2,22,27924688,1.0000,65536,64,0,0
256,1,orthographic,mostly,sparse,8192,15,166.6,170.5,164.1,170.5,8192,0,8192,1,65536,0.2,19,3737384,1.0000,65536,64,0,0
256,1,orthographic,partial,dense,65536,15,688.8,797.9,628.4,797.9,18060,47476,18060,1,65536,0.2,21,12708424,0.2756,30720,64,34,0
256,1,orthographic,partial,sparse,8192,15,70.0,79.4,69.8,79.4,2257,5935,2257,1,65536,0.2,18,1590080,0.2755,30720,64,34,0
256,1,isometric,mostly,dense,65536,15,1897.5,2004.5,1800.4,2004.5,65536,0,65536,1,65536,0.2,22,27924688,1.0000,65536,64,0,0
256,1,isometric,mostly,sparse,8192,15,165.9,192.2,163.2,192.2,8192,0,8192,1,65536,0.2,19,3737384,1.0000,65536,64,0,0
256,1,isometric,partial,dense,65536,15,547.7,593.2,540.4,593.2,16306,49230,16306,1,65536,0.2,20,7461080,0.2488,34816,64,30,0
256,1,isometric,partial,sparse,8192,15,77.7,83.3,77.0,83.3,2060,6132,2060,1,65536,0.2,18,1560136,0.2515,34816,64,30,0
256,2,perspective,mostly,dense,65536,15,3831.3,4017.0,3721.9,4017.0,131072,0,131072,2,131072,0.2,40,55847808,1.0000,131072,128,0,0
256,2,perspective,mostly,sparse,8192,15,337.8,374.3,330.7,374.3,16384,0,16384,2,131072,0.2,34,7473200,1.0000,131072,128,0,0
256,2,perspective,partial,dense,65536,15,2206.2,2320.4,2171.5,2320.4,65080,65992,65080,2,131072,0.2,38,29817200,0.4965,81920,128,48,0
256,2,perspective,partial,sparse,8192,15,203.0,209.7,202.3,209.7,8134,8250,8134,2,131072,0.2,32,3728832,0.4965,81920,128,48,0
256,2,orthographic,mostly,dense,65536,15,3856.2,4112.6,3729.2,4112.6,131072,0,131072,2,131072,0.2,40,55847808,1.0000,131072,128,0,0
256,2,orthographic,mostly,sparse,8192,15,340.6,344.1,339.7,344.1,16384,0,16384,2,131072,0.2,34,7473200,1.0000,131072,128,0,0
256,2,orthographic,partial,dense,65536,15,1338.9,1415.7,1308.6,1415.7,36120,94952,36120,2,131072,0.2,38,25415280,0.2756,61440,128,68,0
256,2,orthographic,partial,sparse,8192,15,140.4,148.7,140.0,148.7,4515,11869,4515,2,131072,0.2,32,3178744,0.2756,61440,128,68,0
256,2,isometric,mostly,dense,65536,15,3847.6,4005.2,3669.4,4005.2,131072,0,131072,2,131072,0.2,40,55847808,1.0000,131072,128,0,0
256,2,isometric,mostly,sparse,8192,15,338.1,372.4,332.1,372.4,16384,0,16384,2,131072,0.2,34,7473200,1.0000,131072,128,0,0
256,2,isometric,partial,dense,65536,15,1093.0,1150.0,1086.0,1150.0,32612,98460,32612,2,131072,0.2,36,14920592,0.2488,69632,128,60,0
256,2,isometric,partial,sparse,8192,15,159.5,194.3,151.9,194.3,4120,12264,4120,2,131072,0.2,32,3118704,0.2515,69632,128,60,0
256,7,perspective,mostly,dense,65536,15,13325.8,14282.9,12925.1,14282.9,458752,0,458752,7,458752,0.4,131,195465840,1.0000,458752,448,0,0
256,7,perspective,mostly,sparse,8192,15,1181.4,1192.5,1179.4,1192.5,57344,0,57344,7,458752,0.3,110,26154712,1.0000,458752,448,0,0
256,7,perspective,partial,dense,65536,15,8030.0,8200.9,7789.5,8200.9,233762,224990,233762,7,458752,0.3,127,135152392,0.5096,296960,448,158,0
256,7,perspective,partial,sparse,8192,15,756.3,791.0,742.8,791.0,29228,28116,29228,7,458752,0.3,106,16900344,0.5097,296960,448,158,0
256,7,orthographic,mostly,dense,65536,15,13105.5,13551.4,12914.8,13551.4,458752,0,458752,7,458752,0.3,131,195465840,1.0000,458752,448,0,0
256,7,orthographic,mostly,sparse,8192,15,1185.5,1208.3,1175.6,1208.3,57344,0,57344,7,458752,0.3,110,26154712,1.0000,458752,448,0,0
256,7,orthographic,partial,dense,65536,15,5482.2,6115.9,4962.1,6115.9,126144,332608,126144,7,458752,0.3,124,88910040,0.2750,243712,448,210,0
256,7,orthographic,partial,sparse,8192,15,561.5,589.8,558.8,589.8,15768,41576,15768,7,458752,0.3,103,11118872,0.2750,243712,448,210,0
256,7,isometric,mostly,dense,65536,15,13660.7,13831.2,12770.2,13831.2,458752,0,458752,7,458752,0.3,131,195465840,1.0000,458752,448,0,0
256,7,isometric,mostly,sparse,8192,15,1182.4,1241.2,1176.0,1241.2,57344,0,57344,7,458752,0.3,110,26154712,1.0000,458752,448,0,0
256,7,isometric,partial,dense,65536,15,3792.1,3839.1,3751.9,3839.1,114124,344628,114124,7,458752,0.3,117,52217848,0.2488,236544,448,217,0
256,7,isometric,partial,sparse,8192,15,534.4,563.3,530.8,563.3,14287,43057,14287,7,458752,0.3,97,7158208,0.2491,236544,448,217,0
512,1,perspective,mostly,dense,262144,9,7806.8,8467.0,7614.2,8467.0,262144,0,262144,1,262144,0.2,24,99616744,1.0000,262144,256,0,0
512,1,perspective,mostly,sparse,32768,9,969.5,1227.8,946.8,1227.8,32768,0,32768,1,262144,0.2,21,14944232,1.0000,262144,256,0,0
512,1,perspective,partial,dense,262144,9,3968.0,4263.0,3933.3,4263.0,129755,132389,129755,1,262144,0.2,23,49709368,0.4950,149504,256,110,0
512,1,perspective,partial,sparse,32768,9,385.3,401.0,384.5,401.0,16220,16548,16220,1,262144,0.2,20,7448200,0.4950,149504,256,110,0
512,1,orthographic,mostly,dense,262144,9,8153.1,8238.5,7658.5,8238.5,262144,0,262144,1,262144,0.2,24,99616744,1.0000,262144,256,0,0
512,1,orthographic,mostly,sparse,32768,9,1029.5,1166.1,965.2,1166.1,32768,0,32768,1,262144,0.2,21,14944232,1.0000,262144,256,0,0
512,1,orthographic,partial,dense,262144,9,3064.0,3235.3,2672.2,3235.3,70656,191488,70656,1,262144,0.2,23,47847824,0.2695,122880,256,136,0
512,1,orthographic,partial,sparse,32768,9,284.2,311.8,282.9,311.8,8832,23936,8832,1,262144,0.2,20,6325224,0.2695,122880,256,136,0
512,1,isometric,mostly,dense,262144,9,8328.3,8898.2,7771.0,8898.2,262144,0,262144,1,262144,0.2,24,99616744,1.0000,262144,256,0,0
512,1,isometric,mostly,sparse,32768,9,1112.0,1240.4,1017.2,1240.4,32768,0,32768,1,262144,0.2,21,14944232,1.0000,262144,256,0,0
512,1,isometric,partial,dense,262144,9,2342.9,2509.0,2227.4,2509.0,64192,197952,64192,1,262144,0.2,22,27924880,0.2449,96256,256,162,0
512,1,isometric,partial,sparse,32768,9,231.4,271.7,227.7,271.7,8067,24701,8067,1,262144,0.2,19,3718576,0.2462,96256,256,162,0
512,2,perspective,mostly,dense,262144,9,16155.9,16485.2,15693.7,16485.2,524288,0,524288,2,524288,0.2,44,199231920,1.0000,524288,512,0,0
512,2,perspective,mostly,sparse,32768,9,2231.6,2723.5,1890.5,2723.5,65536,0,65536,2,524288,0.3,38,29886896,1.0000,524288,512,0,0
512,2,perspective,partial,dense,262144,9,8575.5,9007.5,8378.5,9007.5,259510,264778,259510,2,524288,0.2,42,99417168,0.4950,299008,512,220,0
512,2,perspective,partial,sparse,32768,9,768.6,819.4,764.8,819.4,32439,33097,32439,2,524288,0.2,36,14894680,0.4950,299008,512,220,0
512,2,orthographic,mostly,dense,262144,9,16056.5,16213.8,15778.5,16213.8,524288,0,524288,2,524288,0.2,44,199231920,1.0000,524288,512,0,0
512,2,orthographic,mostly,sparse,32768,9,2007.6,2116.2,1934.6,2116.2,65536,0,65536,2,524288,0.2,38,29886896,1.0000,524288,512,0,0
512,2,orthographic,partial,dense,262144,9,6036.5,6232.7,5955.6,6232.7,141312,382976,141312,2,524288,0.2,42,95694080,0.2695,245760,512,272,0
512,2,orthographic,partial,sparse,32768,9,566.1,604.2,562.2,604.2,17664,47872,17664,2,524288,0.2,36,12648880,0.2695,245760,512,272,0
512,2,isometric,mostly,dense,262144,9,16464.2,37319.7,15599.3,37319.7,524288,0,524288,2,524288,0.2,44,199231920,1.0000,524288,512,0,0
512,2,isometric,mostly,sparse,32768,9,1943.2,2200.8,1908.0,2200.8,65536,0,65536,2,524288,0.2,38,29886896,1.0000,524288,512,0,0
512,2,isometric,partial,dense,262144,9,4739.4,5224.0,4544.8,5224.0,128384,395904,128384,2,524288,0.2,40,55848192,0.2449,192512,512,324,0
512,2,isometric,partial,sparse,32768,9,478.6,522.5,470.9,522.5,16134,49402,16134,2,524288,0.2,34,7435584,0.2462,192512,512,324,0
512,7,perspective,mostly,dense,262144,9,57459.7,62930.3,56161.2,62930.3,1835008,0,1835008,7,1835008,0.3,145,697310232,1.0000,1835008,1792,0,0
512,7,perspective,mostly,sparse,32768,9,8120.0,8509.7,6555.4,8509.7,229376,0,229376,7,1835008,0.3,124,104602648,1.0000,1835008,1792,0,0
512,7,perspective,partial,dense,262144,9,30108.1,30830.5,28092.5,30830.5,930785,904223,930785,7,1835008,0.3,141,470935112,0.5072,1056768,1792,760,0
512,7,perspective,partial,sparse,32768,9,3869.5,4004.0,3796.2,4004.0,116346,113030,116346,7,1835008,0.3,120,67499144,0.5072,1056768,1792,760,0
512,7,orthographic,mostly,dense,262144,9,57296.1,59275.7,56449.0,59275.7,1835008,0,1835008,7,1835008,0.4,145,697310232,1.0000,1835008,1792,0,0
512,7,orthographic,mostly,sparse,32768,9,7894.9,8198.3,6441.9,8198.3,229376,0,229376,7,1835008,0.3,124,104602648,1.0000,1835008,1792,0,0
512,7,orthographic,partial,dense,262144,9,18354.9,19130.4,17188.6,19130.4,496516,1338492,496516,7,1835008,0.3,138,334927792,0.2706,761856,1792,1048,0
512,7,orthographic,partial,sparse,32768,9,1934.7,2003.1,1911.4,2003.1,62061,167315,62061,7,1835008,0.3,117,44305616,0.2706,761856,1792,1048,0
512,7,isometric,mostly,dense,262144,9,57879.4,58858.9,55619.9,58858.9,1835008,0,1835008,7,1835008,0.3,145,697310232,1.0000,1835008,1792,0,0
512,7,isometric,mostly,sparse,32768,9,7753.1,8477.8,6391.7,8477.8,229376,0,229376,7,1835008,0.3,124,104602648,1.0000,1835008,1792,0,0
512,7,isometric,partial,dense,262144,9,15849.2,16117.1,15475.5,16117.1,449604,1385404,449604,7,1835008,0.3,131,195467184,0.2450,677888,1792,1130,0
512,7,isometric,partial,sparse,32768,9,1693.8,1746.9,1661.8,1746.9,56237,173139,56237,7,1835008,0.3,110,25987792,0.2452,677888,1792,1130,0
1000,1,perspective,mostly,dense,1000000,5,29959.1,30108.9,29831.2,30108.9,1000000,0,1000000,1,1000000,0.2,26,394769896,1.0000,1000000,1024,0,0
1000,1,perspective,mostly,sparse,125000,5,3784.0,3806.9,3744.3,3806.9,125000,0,125000,1,1000000,0.2,23,49348680,1.0000,1000000,1024,0,0
1000,1,perspective,partial,dense,1000000,5,15832.2,16668.9,15773.9,16668.9,493588,506412,493588,1,1000000,0.2,25,196899032,0.4936,528960,1024,472,0
1000,1,perspective,partial,sparse,125000,5,2008.4,2051.1,1901.2,2051.1,61699,63301,61699,1,1000000,0.2,22,27925648,0.4936,528960,1024,472,0
1000,1,orthographic,mostly,dense,1000000,5,30523.9,35144.5,30436.5,35144.5,1000000,0,1000000,1,1000000,0.2,26,394769896,1.0000,1000000,1024,0,0
1000,1,orthographic,mostly,sparse,125000,5,4238.5,4288.7,3783.2,4288.7,125000,0,125000,1,1000000,0.2,23,49348680,1.0000,1000000,1024,0,0
1000,1,orthographic,partial,dense,1000000,5,10225.5,10497.5,10053.5,10497.5,269000,731000,269000,1,1000000,0.2,25,179830344,0.2690,330752,1024,701,0
1000,1,orthographic,partial,sparse,125000,5,1205.2,1242.9,1128.6,1242.9,33625,91375,33625,1,1000000,0.2,22,25036736,0.2690,330752,1024,701,0
1000,1,isometric,mostly,dense,1000000,5,36564.9,39142.6,31639.0,39142.6,1000000,0,1000000,1,1000000,0.2,26,394769896,1.0000,1000000,1024,0,0
1000,1,isometric,mostly,sparse,125000,5,5263.0,5534.8,3829.2,5534.8,125000,0,125000,1,1000000,0.2,23,49348680,1.0000,1000000,1024,0,0
1000,1,isometric,partial,dense,1000000,5,8757.3,9363.6,8137.0,9363.6,242060,757940,242060,1,1000000,0.2,24,98091128,0.2421,302080,1024,726,0
1000,1,isometric,partial,sparse,125000,5,2132.9,2255.4,1224.0,2255.4,30339,94661,30339,1,1000000,0.2,21,14575792,0.2427,302080,1024,726,0
1000,2,perspective,mostly,dense,1000000,5,71131.6,81692.5,60643.1,81692.5,2000000,0,2000000,2,2000000,0.2,48,789538224,1.0000,2000000,2048,0,0
1000,2,perspective,mostly,sparse,125000,5,8277.1,10092.1,7740.6,10092.1,250000,0,250000,2,2000000,0.2,42,98695792,1.0000,2000000,2048,0,0
1000,2,perspective,partial,dense,1000000,5,35899.7,38872.9,32989.4,38872.9,987176,1012824,987176,2,2000000,0.2,46,393796496,0.4936,1057344,2048,980,0
1000,2,perspective,partial,sparse,125000,5,4696.8,5027.1,4471.3,5027.1,123397,126603,123397,2,2000000,0.2,40,55849728,0.4936,1057344,2048,980,0
1000,2,orthographic,mostly,dense,1000000,5,72266.4,77124.6,68877.4,77124.6,2000000,0,2000000,2,2000000,0.2,48,789538224,1.0000,2000000,2048,0,0
1000,2,orthographic,mostly,sparse,125000,5,8360.9,12028.5,8039.2,12028.5,250000,0,250000,2,2000000,0.2,42,98695792,1.0000,2000000,2048,0,0
1000,2,orthographic,partial,dense,1000000,5,20299.7,20865.0,19484.5,20865.0,538000,1462000,538000,2,2000000,0.2,46,359659120,0.2690,642048,2048,1421,0
1000,2,orthographic,partial,sparse,125000,5,2896.5,4351.1,2571.8,4351.1,67250,182750,67250,2,2000000,0.2,40,50071904,0.2690,642048,2048,1421,0
1000,2,isometric,mostly,dense,1000000,5,66652.0,72771.8,63019.7,72771.8,2000000,0,2000000,2,2000000,0.2,48,789538224,1.0000,2000000,2048,0,0
1000,2,isometric,mostly,sparse,125000,5,8072.8,10374.8,7948.2,10374.8,250000,0,250000,2,2000000,0.2,42,98695792,1.0000,2000000,2048,0,0
1000,2,isometric,partial,dense,1000000,5,16784.5,17151.1,16711.0,17151.1,484120,1515880,484120,2,2000000,0.2,44,196180688,0.2421,605184,2048,1454,0
1000,2,isometric,partial,sparse,125000,5,2270.8,3555.0,2263.8,3555.0,60678,189322,60678,2,2000000,0.2,38,29150016,0.2427,605184,2048,1454,0
1000,7,perspective,mostly,dense,1000000,5,259282.7,281578.0,240475.8,281578.0,7000000,0,7000000,7,7000000,0.4,159,2763382296,1.0000,7000000,7168,0,0
1000,7,perspective,mostly,sparse,125000,5,43561.9,50519.5,36839.6,50519.5,875000,0,875000,7,7000000,0.4,138,345433784,1.0000,7000000,7168,0,0
1000,7,perspective,partial,dense,1000000,5,118698.4,133540.8,108216.2,133540.8,3541993,3458007,3541993,7,7000000,0.3,155,1863039632,0.5060,3795136,7168,3336,0
1000,7,perspective,partial,sparse,125000,5,17013.8,17312.4,14983.4,17312.4,442743,432257,442743,7,7000000,0.3,134,252679432,0.5060,3795136,7168,3336,0
1000,7,orthographic,mostly,dense,1000000,5,236448.5,257163.7,219295.9,257163.7,7000000,0,7000000,7,7000000,0.3,159,2763382296,1.0000,7000000,7168,0,0
1000,7,orthographic,mostly,sparse,125000,5,41488.8,51474.5,29472.8,51474.5,875000,0,875000,7,7000000,0.4,138,345433784,1.0000,7000000,7168,0,0
1000,7,orthographic,partial,dense,1000000,5,68405.0,71232.0,66649.8,71232.0,1881521,5118479,1881521,7,7000000,0.3,152,1258693256,0.2688,2347264,7168,4872,0
1000,7,orthographic,partial,sparse,125000,5,10318.2,10471.2,9636.8,10471.2,235190,639810,235190,7,7000000,0.3,131,175222056,0.2688,2347264,7168,4872,0
1000,7,isometric,mostly,dense,1000000,5,226838.7,283753.7,214537.6,283753.7,7000000,0,7000000,7,7000000,0.3,159,2763382296,1.0000,7000000,7168,0,0
1000,7,isometric,mostly,sparse,125000,5,54887.5,73806.1,47865.8,73806.1,875000,0,875000,7,7000000,0.3,138,345433784,1.0000,7000000,7168,0,0
1000,7,isometric,partial,dense,1000000,5,58020.8,69525.2,54717.0,69525.2,1701429,5298571,1701429,7,7000000,0.3,145,687163832,0.2431,2127872,7168,5084,0
1000,7,isometric,partial,sparse,125000,5,8221.2,8420.4,7845.6,8420.4,212757,662243,212757,7,7000000,0.3,124,102081936,0.2432,2127872,7168,5084,0
```

## Debug lines

Measured 2026-10-24, 12:08 PM America/Chicago, on Apple M5, macOS arm64, `rustc 1.99.0`. Release. Command: `cargo run --release -p engine-renderer --bin debug_draw_bench`. The binary is `crates/engine-renderer/src/bin/debug_draw_bench.rs`. It refuses a debug build. No GPU, and no new dependency.

Generation is the `DebugDraw::line` calls that build a new list. Extraction is `extract_scene` after that list is already a resource, including dropping the `RenderScene`. Four warmup calls, then the median of 11. The viewport is 1280×720. Cameras are orthographic and look at the segments. `alloc_bytes` is one extra extract after the timed samples. It sums `Layout::size` for every `alloc` and `realloc` and does not subtract frees. The same reading as the tilemap column: cumulative traffic, not resident memory, not a GPU allocation.

`implied_draws` is not a GPU measurement. It is 0 when there are no segments and otherwise one line draw per camera. The GPU tests check that count. This renderer does not write timestamps (`timestamp_writes` is `None`), so GPU time is not claimed.

`DebugSegment` in this build is 40 bytes (two `Vec3` and one `[f32; 4]`). Ten thousand segments are 400,000 bytes. The one-camera row's 401,872 bytes are that copy plus the 1,872 bytes an empty one-camera extract already records. The two-camera row adds 656 bytes for the second pass (402,528 − 401,872), not a second copy of the segments. `alloc_count` stays 3 from 100 segments upward: the segment vector is one allocation.

An absent `DebugDraw` and an empty one record the same traffic (1,872 bytes and 2 allocations with one camera; 2,528 bytes and 2 allocations with two). Their extract medians are 1.0–1.4 µs, the same band as a scene that has cameras and no segments. Unused debug data does not add an extract allocation. The renderer still creates the line pipeline and a 4,096-byte vertex buffer when the `SceneRenderer` is constructed. An empty frame does not write that buffer and does not draw.

| Segments | Cameras | Vertices | Implied draws | Generation | Extraction | Alloc bytes | Alloc count |
|---|---:|---:|---:|---:|---:|---:|---:|
| absent | 1 | 0 | 0 | not timed | 1.17 µs | 1,872 | 2 |
| empty | 1 | 0 | 0 | not timed | 1.04 µs | 1,872 | 2 |
| absent | 2 | 0 | 0 | not timed | 1.42 µs | 2,528 | 2 |
| empty | 2 | 0 | 0 | not timed | 1.42 µs | 2,528 | 2 |
| 0 | 1 | 0 | 0 | 0.04 µs | 1.00 µs | 1,872 | 2 |
| 0 | 2 | 0 | 0 | 0.04 µs | 1.46 µs | 2,528 | 2 |
| 100 | 1 | 200 | 1 | 1.38 µs | 1.17 µs | 5,872 | 3 |
| 100 | 2 | 200 | 2 | 1.25 µs | 1.63 µs | 6,528 | 3 |
| 1,000 | 1 | 2,000 | 1 | 10.4 µs | 2.50 µs | 41,872 | 3 |
| 1,000 | 2 | 2,000 | 2 | 10.4 µs | 2.71 µs | 42,528 | 3 |
| 10,000 | 1 | 20,000 | 1 | 40.9 µs | 5.08 µs | 401,872 | 3 |
| 10,000 | 2 | 20,000 | 2 | 40.3 µs | 5.13 µs | 402,528 | 3 |

The 0-segment generation time is an empty loop. It is not a claim that constructing the resource is free. The 10,000-segment generation time includes growing a new vector from empty. These are medians of a warm process. They are not a budget, and they are not a GPU frame.

```
segments cameras vertices implied_draws gen_ns extract_ns alloc_bytes alloc_count
absent 1 0 0 0 1167 1872 2
empty 1 0 0 0 1042 1872 2
absent 2 0 0 0 1417 2528 2
empty 2 0 0 0 1417 2528 2
0 1 0 0 42 1000 1872 2
0 2 0 0 41 1458 2528 2
100 1 200 1 1375 1167 5872 3
100 2 200 2 1250 1625 6528 3
1000 1 2000 1 10417 2500 41872 3
1000 2 2000 2 10417 2708 42528 3
10000 1 20000 1 40875 5083 401872 3
10000 2 20000 2 40333 5125 402528 3
```

## Render scale

Measured 2026-10-24 on Apple M5, Metal, macOS arm64, `rustc 1.99.0`. Release. The CPU log was written at 1:00 PM America/Chicago and the GPU log at 1:02 PM America/Chicago. The measured code is `82972c2` on `feat/m6-render-scale`. The following commit, `b26dd3a`, only corrects rustdoc links. No new dependency. Rendering draws were not changed. `SceneRenderer::instance_buffer_capacity_bytes` only reports reserved buffer bytes.

Commands:

```
cargo run --release -p engine-renderer --bin render_scale_bench
cargo run --release -p engine-renderer-wgpu --example render_scale_gpu
DIMORA_SCALE_CASES=sprite-opaque-1k,mesh-shared-100 cargo run --release -p engine-renderer --bin render_scale_bench
DIMORA_SCALE_LIST=1 cargo run --release -p engine-renderer --bin render_scale_bench
```

The CPU binary is `crates/engine-renderer/src/bin/render_scale_bench.rs`. The GPU example is `crates/engine-renderer-wgpu/examples/render_scale_gpu.rs`. Both refuse a debug build. Scenes are `engine_renderer::bench_scene`. That module is the benchmark catalog, not a gameplay API and not a prelude export. A later milestone adds a case there; it does not replace the harness. The lane scene is the hybrid world those later cases should keep.

`extract_median_ns` is `extract_scene` plus dropping the `RenderScene`. Sorting is inside that time. `propagate_median_ns` is a later `propagate_transforms` on an already-propagated world. `motion_prepare_median_ns` is zero except `sprite-opaque-10k-move`, where each sample adds 0.01 to every sprite's local X and then propagates. That prepare is outside the extract time. Two unrecorded extracts of the first case run before the table. `first_extract_ns` is the call before that row's warmup.

Rows whose authored sprites or meshes, times the camera count, reach 100,000 use 1 warmup and 7 samples. Smaller CPU rows use 4 warmup and 11 samples. The median is the middle sample. Nearest-rank p95 on 7 or 11 samples is the slowest sample. Stable CPU rows have a p95 close to the median. The narrative uses medians. The first opaque rows (`sprite-opaque-1k` at 761 µs, `sprite-opaque-10k` at 2.45 ms) are higher than later rows of the same size (about 163 µs and 1.67 ms). Those two medians are not the slope.

`alloc_bytes` sums `Layout::size` for every `alloc` and `realloc` during one later call and does not subtract frees. `peak_extra_bytes` is the high water of outstanding bytes during that call, minus the outstanding bytes at the start. `retained_bytes` is the capacity of the returned scene's vectors. None of these is process RSS or a Metal heap. The tilemap column in the sections above is the same kind of traffic. It is not a claim that a sprite crowd allocates like a million-tile map.

GPU `encode_*` is `SceneRenderer::render` after a poll, so the queue is idle. That call records command buffers and submits them. It does not map a readback and it does not present. `completion_*` is the following `device.poll(wait_indefinitely)`. That waits until submitted work finishes. It includes GPU execution and driver synchronization. It is not a timestamp query inside the pass. This renderer still sets `timestamp_writes` to `None`. Completion p95 often jumps to several milliseconds while the median stays under half a millisecond, so completion is not a GPU budget. `first_encode_ns` is the first render after pipeline creation and includes uploads and instance-buffer growth. Each GPU case builds a new `SceneRenderer`. Steady samples follow that frame plus warmup (1 and 7 at 100,000 authored objects times cameras, otherwise 2 and 11). `steady_buffer_grows` was 0 on every row. The color target is `Rgba8Unorm`, 1280×720, `RENDER_ATTACHMENT` only.

Visible and batch columns are summed across cameras. `sprites_visible` counts tile quads as well as authored sprites. `transparent_batches` overlaps the blended part of `sprite_batches`. Textures are 4×4 solids. This is not a texture-bandwidth test. Grid sprite materials are double-sided. Mesh-field materials use the default cull mode. The lane's crate material uses the sprite helper, so it is double-sided. Cuboid index count is 36. The reference sphere is 1,056. Eight distinct mesh assets differ by asset id, not by a measured vertex-cost sweep.

### Sprite extraction

One shared opaque material stays one batch at 1,000, 10,000, and 100,000 sprites. Mask matches that time. Blend stays one batch when every sprite shares the material, and costs more at the high count. Eight opaque materials are eight batches. Eight blended materials, interleaved in depth order, are 1,000 batches at 1,000 sprites. Mixed `Billboard`, `Upright`, and `Fixed` stay one opaque batch. `CoplanarPriority` levels 0 through 7 stay one opaque batch. A moving 10,000-sprite extract matches the static extract. One thousand debug segments add 40,000 bytes of retained capacity and do not move the 1,000-sprite extract.

| Scene | Visible | Culled | Batches | Extract | Propagate | Alloc bytes | Alloc count | Retained |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| mask 1k | 1,000 | 0 | 1 | 163 µs | 11 µs | 872,816 | 1,022 | 156,464 |
| opaque 10k, later unlit row | 10,000 | 0 | 1 | 1.67 ms | 102 µs | 12,850,736 | 10,030 | 2,491,184 |
| mask 10k | 10,000 | 0 | 1 | 1.67 ms | 104 µs | 12,850,736 | 10,030 | 2,491,184 |
| blend 10k | 10,000 | 0 | 1 | 2.36 ms | 102 µs | 18,515,280 | 10,044 | 2,491,376 |
| legacy cutout 10k | 10,000 | 0 | 1 | 1.05 ms | 103 µs | 12,850,736 | 10,030 | 2,491,184 |
| opaque, 8 materials, 10k | 10,000 | 0 | 8 | 1.92 ms | 103 µs | 12,851,056 | 10,031 | 2,491,344 |
| blend, 8 materials, 1k | 1,000 | 0 | 1,000 | 206 µs | 11 µs | 1,411,920 | 1,048 | 246,416 |
| orientations 10k | 10,000 | 0 | 1 | 1.66 ms | 103 µs | 12,850,736 | 10,030 | 2,491,184 |
| coplanar levels 10k | 10,000 | 0 | 1 | 1.71 ms | 103 µs | 12,850,736 | 10,030 | 2,491,184 |
| sparse, camera frames all, 10k | 10,000 | 0 | 1 | 1.69 ms | 103 µs | 12,850,736 | 10,030 | 2,491,184 |
| sparse, partial, 10k | 1,596 | 8,404 | 1 | 1.57 ms | 103 µs | 7,215,184 | 10,027 | 312,112 |
| dense, partial, 10k | 1,800 | 8,200 | 1 | 1.56 ms | 103 µs | 7,246,192 | 10,027 | 312,112 |
| sparse, tight, 10k | 20 | 9,980 | 1 | 1.54 ms | 102 µs | 6,359,728 | 10,020 | 5,680 |
| moving 10k | 10,000 | 0 | 1 | 1.67 ms | 103 µs | 12,850,736 | 10,030 | 2,491,184 |
| hierarchy 1k | 1,000 | 0 | 1 | 192 µs | 25 µs | 907,640 | 1,023 | 156,464 |
| opaque 100k | 100,000 | 0 | 1 | 19.06 ms | 1.53 ms | 99,280,984 | 100,036 | 19,923,760 |
| mask 100k | 100,000 | 0 | 1 | 19.61 ms | 1.57 ms | 99,280,984 | 100,036 | 19,923,760 |
| blend 100k | 100,000 | 0 | 1 | 29.16 ms | 1.75 ms | 143,720,784 | 100,053 | 19,923,952 |

The move row's prepare median is 148 µs. That is the nudge plus propagation. Extract after those moves matches the static 10,000-sprite row. Hierarchy is 100 roots of 10 children. Propagation is about twice the flat 1,000-sprite propagate, and extract stays in the same band as a flat 1,000.

A 16.7 ms frame budget is context, not an adopted engine budget. Ten thousand opaque sprites fit on this machine: about 1.67 ms to extract. One hundred thousand opaque or mask sprites spend about 19 ms in extract alone. One hundred thousand blended sprites spend about 29 ms. Rejecting 9,980 of 10,000 sprites leaves extract at 1.54 ms. The retained scene drops from 2.49 MB to 5,680 bytes. `alloc_count` stays about 10,020. Frustum rejection does not skip the prepare walk.

`AncestorVisibility::hides` builds a `Vec` on every call, including a parentless visible entity. `alloc_count` tracks that: about 10,030 for 10,000 sprites and 100,036 for 100,000, including the tight cull. The share of the 1.5 ms that belongs to those allocations was not timed apart from the walk.

Opaque and mask 10,000-sprite retained capacity is 2,491,184 bytes. `SpriteInstance` is 152 bytes, and 16,384 × 152 is 2,490,368. The rest is the other scene vectors. One hundred thousand sprites retain 19,923,760 bytes, which is 131,072 × 152 plus that same remainder. Seven cameras retain exactly seven times the one-camera figure.

### Mesh extraction

| Scene | Visible | Culled | Draws | Extract | Alloc bytes | Alloc count | Retained |
|---|---:|---:|---:|---:|---:|---:|---:|
| shared cuboid 100 | 100 | 0 | 1 | 12.8 µs | 93,264 | 115 | 18,064 |
| sphere 100 | 100 | 0 | 1 | 12.7 µs | 93,264 | 115 | 18,064 |
| shared cuboid 1k | 1,000 | 0 | 1 | 137 µs | 774,896 | 1,021 | 139,920 |
| eight assets 1k | 1,000 | 0 | 8 | 147 µs | 774,896 | 1,021 | 139,920 |
| eight materials 1k | 1,000 | 0 | 8 | 140 µs | 774,896 | 1,021 | 139,920 |
| partial 1k | 210 | 790 | 1 | 131 µs | 458,560 | 1,019 | 35,472 |
| shared cuboid 10k | 10,000 | 0 | 1 | 1.40 ms | 11,380,016 | 10,029 | 2,228,880 |

CPU extract does not read the index buffer. A 1,056-index sphere matches a 36-index cuboid at 100 instances. Eight assets or eight materials add draws and do not add a measurable extract cost at 1,000 instances. Ten thousand shared meshes are one draw and 1.40 ms. `MeshInstance` is 136 bytes.

### Mixed lane

The lane is a constructed street, not a shipped game. A 48×48 tile ground, 160 upright opaque actors, 48 upright mask trees, and 24 billboard blended glows. With meshes: 16 crates and 6 buildings, one cuboid asset and one opaque material. Default lights are 4 point lights. Default camera is perspective, distance 55, 50° field of view. Orthographic and isometric rows use height 52. `sprites_visible` of 2,536 is 232 authored sprites plus 2,304 tile quads.

| Scene | Cameras | Visible sprites | Visible meshes | Tile quads | Sprite batches | Mesh draws | Extract |
|---|---:|---:|---:|---:|---:|---:|---:|
| sprites only | 1 | 2,536 | 0 | 2,304 | 4 | 0 | 87 µs |
| mixed, perspective | 1 | 2,536 | 22 | 2,304 | 4 | 1 | 90 µs |
| mixed, orthographic | 1 | 2,536 | 22 | 2,304 | 4 | 1 | 89 µs |
| mixed, isometric | 1 | 2,536 | 22 | 2,304 | 4 | 1 | 89 µs |
| mixed, 2 cameras | 2 | 5,072 | 44 | 4,608 | 8 | 2 | 147 µs |
| mixed, 7 cameras | 7 | 17,713 | 154 | 16,128 | 28 | 7 | 438 µs |

Adding 22 meshes changes extract by a few microseconds. That comparison is 0 versus 22 meshes. The mesh field above is the crowd. Seven cameras visit 2,304 × 7 tile quads. One isometric camera of the seven leaves 39 sprite quads culled; the other lane rows cull none. All four chunks of the 48×48 map are accepted by a single camera.

### Lights

Point lights, except the unit test that also keeps one directional when 20 points would otherwise fill the 16 slots. `sprite-lights-32-10k` authors 32 and selects 16. CPU extract stays at 1.69 ms for 0, 4, 16, and 32 lights over 10,000 opaque sprites. Mesh 1,000 stays at 133–137 µs for 4 and 16 lights. The lane stays at 88–89 µs for 0, 4, and 16 lights. Selection cost at these counts is inside the noise of the extract. Upload of the fixed 16-light uniform is inside encode, below. These rows do not measure shader lighting, normal maps, or shadows. Those are not implemented.

### Cameras

| Scene | Cameras | Visible sprites | Extract | Retained |
|---|---:|---:|---:|---:|
| opaque 10k, one orthographic | 1 | 10,000 | 1.67 ms | 2,491,184 |
| opaque 10k, perspective | 1 | 10,000 | 1.69 ms | 2,491,184 |
| opaque 10k, isometric | 1 | 10,000 | 1.69 ms | 2,491,184 |
| opaque 10k, two | 2 | 20,000 | 2.08 ms | 4,982,368 |
| opaque 10k, seven orthographic | 7 | 70,000 | 4.07 ms | 17,438,288 |
| opaque 10k, seven cycling ortho, perspective, iso | 7 | 70,000 | 4.05 ms | 17,438,288 |
| offscreen plus surface, 1k | 2 | 2,002 | 204 µs | 313,024 |
| two surface cameras, 1k | 2 | 2,000 | 203 µs | 312,928 |

Seven cameras take about 2.4× one camera's extract and exactly 7× the retained instance capacity. The prepare walk is shared. The instance vectors are not. Cycling projections matches seven orthographic cameras. An offscreen 1280×720 target plus the surface camera matches two surface cameras on the CPU. The writing camera cannot sample its own target (`missing_targets` is 1 in the unit test). The surface pair has no target.

### GPU encode and submit

Apple M5, Metal. Encode is CPU recording and submit. Completion is the driver wait and is not a pass timestamp.

| Scene | Encode | Draws | World | Clear | Instances | Buffer capacity | Render alloc bytes |
|---|---:|---:|---:|---:|---:|---:|---:|
| opaque 1k | 48 µs | 1 | 1 | 0 | 1,000 | 155,648 | 121,637 |
| opaque 10k | 159 µs | 1 | 1 | 0 | 10,000 | 2,121,728 | 1,129,637 |
| opaque 100k | 1.40 ms | 1 | 1 | 0 | 100,000 | 16,801,792 | 11,209,637 |
| mask 10k | 158 µs | 1 | 1 | 0 | 10,000 | 2,121,728 | 1,129,637 |
| blend 1k | 44 µs | 1 | 1 | 0 | 1,000 | 155,648 | 121,637 |
| blend 10k | 151 µs | 1 | 1 | 0 | 10,000 | 2,121,728 | 1,129,637 |
| blend 100k | 1.38 ms | 1 | 1 | 0 | 100,000 | 16,801,792 | 11,209,637 |
| opaque, 8 materials, 10k | 174 µs | 8 | 8 | 0 | 10,000 | 2,121,728 | 1,132,741 |
| blend, 8 materials, 1k | 281 µs | 1,000 | 1,000 | 0 | 1,000 | 155,648 | 596,037 |
| dense partial 10k | 54 µs | 1 | 1 | 0 | 1,800 | 286,720 | 211,461 |
| sparse tight 10k | 30 µs | 1 | 1 | 0 | 20 | 28,672 | 12,101 |
| unlit opaque 10k | 150 µs | 1 | 1 | 0 | 10,000 | 2,121,728 | 1,129,861 |
| 16 lights, opaque 10k | 151 µs | 1 | 1 | 0 | 10,000 | 2,121,728 | 1,129,861 |
| seven cameras, opaque 10k | 1.21 ms | 13 | 7 | 6 | 70,000 | 2,121,728 | 7,914,019 |
| shared mesh 100 | 35 µs | 1 | 1 | 0 | 100 | 40,960 | 20,405 |
| sphere 100 | 35 µs | 1 | 1 | 0 | 100 | 40,960 | 20,573 |
| shared mesh 1k | 90 µs | 1 | 1 | 0 | 1,000 | 155,648 | 106,805 |
| shared mesh 10k | 592 µs | 1 | 1 | 0 | 10,000 | 1,073,152 | 970,805 |
| eight mesh assets 1k | 92 µs | 8 | 8 | 0 | 1,000 | 155,648 | 108,765 |
| lane mixed | 78 µs | 5 | 5 | 0 | 2,558 | 319,488 | 300,893 |
| lane, 7 cameras | 615 µs | 41 | 35 | 6 | 17,867 | 319,488 | 2,101,883 |
| lane, 16 lights | 77 µs | 5 | 5 | 0 | 2,558 | 319,488 | 300,893 |
| offscreen 1k | 87 µs | 4 | 4 | 0 | 2,002 | 155,648 | 246,110 |
| two surface cameras, 1k | 90 µs | 3 | 2 | 1 | 2,000 | 155,648 | 244,938 |

Sixteen lights encode in the same 150 µs as the unlit 10,000-sprite row. The lane with 16 lights encodes in 77 µs against 78 µs with 4. That is not a measured GPU lighting cost. Completion medians for those pairs sit in a noisy band (unlit 255 µs, 16 lights 396 µs, both with multi-millisecond p95).

Interleaved blended materials are the draw-call case. CPU extract stays at 206 µs, the same as one blended batch. Encode moves from 44 µs to 281 µs, and the draw count from 1 to 1,000. Eight opaque materials stay at 8 draws and move encode from 159 µs to 174 µs. Coplanar levels and mixed orientations stay at one draw.

Instance-buffer capacity is the seven buffers, each starting at 4,096 bytes and growing to the next power of two. Empty layers do not shrink. A 100,000-sprite frame reserves 16,777,216 plus 6 × 4,096 = 16,801,792. A 20-sprite frame never grows, so capacity stays 7 × 4,096 = 28,672. Seven cameras reuse one camera's buffer: the 10,000-sprite seven-camera row stays at 2,121,728. The CPU upload vector is not shared. Render `alloc_bytes` at 10,000 sprites is 1,129,637, and at seven cameras is 7,914,019. Steady samples do not grow the GPU buffers. They still upload.

`texture_creates` counts from `SceneRenderer::new`, including the two placeholders. One scene texture records 3. Eight material textures record 10. The lane's albedo plus tile texture records 4. `base_upload_bytes` is 72, 520, and 136 on those same rows. `render_target_creates` is 1 on the offscreen row and 0 on the two-surface row. Mesh cache length is 1 for a shared cuboid, 8 for eight assets, and 1 for the lane. The offscreen row's four world draws are the crowd plus the sprite that samples the target. The two-surface row is two world draws and one viewport clear.

A 100-instance sphere overlaps the cuboid's 35 µs encode. Index count is not a measured GPU limit at that size. Ten thousand meshes encode in 592 µs against 1.40 ms of CPU extract. Ten thousand sprites encode in 159 µs against 1.67 ms of extract. On these rows the CPU extract is the larger part of the measured frame. Pure GPU pass time remains unmeasured.

### What this says about KI-31

The full-view million-tile case still holds: one orthographic camera records hundreds of megabytes of allocator traffic and keeps 152 MiB of instance-vector capacity, and seven cameras keep 1,064 MiB. Sprite retention has the same shape at a different size. Ten thousand sprites keep 2.49 MB per camera and extract in about 1.7 ms. That does not pull the tile reuse forward. KI-31 stays deferred. The `hides` allocation is a separate, smaller follow-up. It is not that tile-vector reuse.

### Later cases, not built here

The catalog can gain a case without a new harness. Not measured, because the systems are not in the engine: sprite-sheet animation, skeletal 2D pose evaluation, skinned 3D characters, directional sprite animation, sprite and mesh shadows, an editor viewport, selection, asset hot reload, and scene instantiation. Debug drawing already has its own section. The lane remains the hybrid scene those cases should extend.

### Limits

One machine, Apple M5 Metal. No Vulkan, DirectX, OpenGL, or lavapipe numbers. No GPU timestamp. No process RSS. No Metal heap size. No separate sort timer. No 100,000-sprite tight camera. No texture-bandwidth sweep. No mesh-complexity sweep past 100 spheres. The lane is not a 10,000-mesh scene. Completion p95 is not a budget. The first two opaque CPU medians are noisy. M6 is not complete.

### CPU record

```
render_scale_bench viewport=1280x720 cuboid_indices=36 sphere_indices=1056 sprite_instance_bytes=152 mesh_instance_bytes=136
label sprites meshes lights tiles cameras passes visible_sprites culled_sprites visible_meshes culled_meshes tiles_visited chunks_tested chunks_rejected chunks_rebuilt sprite_batches mesh_draws transparent_batches lights_min lights_max missing_assets debug_segments first_extract_ns extract_median_ns extract_p95_ns propagate_median_ns motion_prepare_median_ns alloc_bytes alloc_count peak_extra_bytes retained_bytes samples
sprite-opaque-1k 1000 0 0 0 1 1 1000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 429958 761083 1016042 57500 0 872816 1022 497936 156464 11
sprite-opaque-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 6087792 2448542 3120208 214125 0 12850736 10030 7026896 2491184 11
sprite-opaque-100k 100000 0 0 0 1 1 100000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 18240958 19060459 19531667 1529667 0 99280984 100036 52041976 19923760 7
sprite-mask-1k 1000 0 0 0 1 1 1000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 185416 162625 163875 10750 0 872816 1022 497936 156464 11
sprite-mask-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1891125 1674500 1692958 103875 0 12850736 10030 7026896 2491184 11
sprite-mask-100k 100000 0 0 0 1 1 100000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 19680792 19610875 19863000 1571792 0 99280984 100036 52041976 19923760 7
sprite-blend-1k 1000 0 0 0 1 1 1000 0 0 0 0 0 0 0 1 0 1 0 0 0 0 218000 202666 210250 10625 0 1232400 1032 530320 156656 11
sprite-blend-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 1 0 0 0 0 2795875 2355375 2509875 102125 0 18515280 10044 8259760 2491376 11
sprite-blend-100k 100000 0 0 0 1 1 100000 0 0 0 0 0 0 0 1 0 1 0 0 0 0 28939125 29162875 30271958 1746833 0 143720784 100053 66062512 19923952 7
sprite-legacy-cutout-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1655625 1045625 1092542 102667 0 12850736 10030 7026896 2491184 11
sprite-opaque-materials-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 8 0 0 0 0 0 0 1932375 1918125 2008459 102833 0 12851056 10031 7026896 2491344 11
sprite-blend-materials-1k 1000 0 0 0 1 1 1000 0 0 0 0 0 0 0 1000 0 1000 0 0 0 0 218292 205584 224542 10667 0 1411920 1048 608080 246416 11
sprite-orient-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1660333 1662917 1744584 102625 0 12850736 10030 7026896 2491184 11
sprite-coplanar-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1685167 1705625 1777833 102959 0 12850736 10030 7026896 2491184 11
sprite-sparse-mostly-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1700583 1690250 1793041 103166 0 12850736 10030 7026896 2491184 11
sprite-sparse-partial-10k 10000 0 0 0 1 1 1596 8404 0 0 0 0 0 0 1 0 0 0 0 0 0 1574167 1571584 1673167 102625 0 7215184 10027 3570416 312112 11
sprite-dense-partial-10k 10000 0 0 0 1 1 1800 8200 0 0 0 0 0 0 1 0 0 0 0 0 0 1570667 1564458 1592667 102916 0 7246192 10027 3601424 312112 11
sprite-sparse-tight-10k 10000 0 0 0 1 1 20 9980 0 0 0 0 0 0 1 0 0 0 0 0 0 1652292 1538834 1578292 102250 0 6359728 10020 3021552 5680 11
sprite-opaque-10k-unlit 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1750292 1673583 1708166 101625 0 12850736 10030 7026896 2491184 11
sprite-opaque-10k-move 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1689584 1673334 1767959 102875 147750 12850736 10030 7026896 2491184 11
sprite-hierarchy-1k 1000 0 0 0 1 1 1000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 197209 192125 210125 25334 0 907640 1023 532760 156464 11
sprite-opaque-1k-debug 1000 0 0 0 1 1 1000 0 0 0 0 0 0 0 1 0 0 0 0 0 1000 156709 162459 166375 10792 0 912816 1023 497936 196464 11
sprite-lights-4-10k 10000 0 4 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 4 4 0 0 1772083 1698375 1783959 102250 0 12851280 10036 7027104 2491392 11
sprite-lights-16-10k 10000 0 16 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 16 16 0 0 1698000 1688291 1799584 102792 0 12853536 10050 7027728 2492016 11
sprite-lights-32-10k 10000 0 32 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 16 16 0 0 1707584 1687542 1717291 102542 0 12856544 10067 7028560 2492848 11
sprite-cam-2-10k 10000 0 0 0 2 2 20000 0 0 0 0 0 0 0 2 0 0 0 0 0 0 2372250 2079583 2169083 102500 0 19351680 10045 9518080 4982368 11
sprite-cam-7-10k 10000 0 0 0 7 7 70000 0 0 0 0 0 0 0 7 0 0 0 0 0 0 6223750 4074167 4262250 103000 0 51858832 10121 21975216 17438288 11
sprite-cam-persp-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1710084 1687791 1715459 102750 0 12850736 10030 7026896 2491184 11
sprite-cam-iso-10k 10000 0 0 0 1 1 10000 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1696291 1692166 1740166 102625 0 12850736 10030 7026896 2491184 11
sprite-cam-cycle-7-10k 10000 0 0 0 7 7 70000 0 0 0 0 0 0 0 7 0 0 0 0 0 0 4041458 4054750 4172125 102167 0 51858832 10121 21975216 17438288 11
mesh-shared-100 0 100 0 0 1 1 0 0 100 0 0 0 0 0 0 1 0 0 0 0 0 24500 12834 13875 1292 0 93264 115 53360 18064 11
mesh-shared-1k 0 1000 0 0 1 1 0 0 1000 0 0 0 0 0 0 1 0 0 0 0 0 126875 137125 143834 9916 0 774896 1021 440976 139920 11
mesh-shared-10k 0 10000 0 0 1 1 0 0 10000 0 0 0 0 0 0 1 0 0 0 0 0 2027375 1395125 1473125 95209 0 11380016 10029 6211536 2228880 11
mesh-distinct-1k 0 1000 0 0 1 1 0 0 1000 0 0 0 0 0 0 8 0 0 0 0 0 154583 146959 151791 9875 0 774896 1021 440976 139920 11
mesh-materials-1k 0 1000 0 0 1 1 0 0 1000 0 0 0 0 0 0 8 0 0 0 0 0 143334 140375 142166 10041 0 774896 1021 440976 139920 11
mesh-partial-1k 0 1000 0 0 1 1 0 0 210 790 0 0 0 0 0 1 0 0 0 0 0 132416 130917 131334 9834 0 458560 1019 229088 35472 11
mesh-sphere-100 0 100 0 0 1 1 0 0 100 0 0 0 0 0 0 1 0 0 0 0 0 17417 12667 13708 1292 0 93264 115 53360 18064 11
mesh-lights-4-1k 0 1000 4 0 1 1 0 0 1000 0 0 0 0 0 0 1 0 4 4 0 0 136417 137416 150416 10167 0 775440 1027 441184 140128 11
mesh-lights-16-1k 0 1000 16 0 1 1 0 0 1000 0 0 0 0 0 0 1 0 16 16 0 0 142791 133042 137458 9541 0 777696 1041 441808 140752 11
lane-sprites 232 0 4 2304 1 1 2536 0 0 0 2304 4 0 0 4 0 1 4 4 0 0 99125 87083 90458 2708 0 1854228 290 1076160 677792 11
lane-mixed 232 22 4 2304 1 1 2536 0 22 0 2304 4 0 0 4 1 1 4 4 0 0 94042 89542 90917 3291 0 1879220 321 1085632 682144 11
lane-ortho 232 22 4 2304 1 1 2536 0 22 0 2304 4 0 0 4 1 1 4 4 0 0 92875 88542 90875 3292 0 1879220 321 1085632 682144 11
lane-iso 232 22 4 2304 1 1 2536 0 22 0 2304 4 0 0 4 1 1 4 4 0 0 96625 89083 91417 3292 0 1879220 321 1085632 682144 11
lane-cameras-2 232 22 4 2304 2 2 5072 0 44 0 4608 8 0 0 8 2 2 4 4 0 0 155208 147459 154208 3417 0 3645336 368 1767776 1364288 11
lane-cameras-7 232 22 4 2304 7 7 17713 39 154 0 16128 28 0 0 28 7 7 4 4 0 0 455208 437792 455833 3333 0 12472420 604 5179712 4775008 11
lane-lights-0 232 22 0 2304 1 1 2536 0 22 0 2304 4 0 0 4 1 1 0 0 0 0 93750 87667 98083 2709 0 1878676 315 1085424 681936 11
lane-lights-16 232 22 16 2304 1 1 2536 0 22 0 2304 4 0 0 4 1 1 16 16 0 0 87209 89250 95250 3458 0 1881476 335 1086256 682768 11
offscreen-1k 1001 0 0 0 2 2 2002 0 0 0 0 0 0 0 4 0 0 0 0 0 0 211291 204375 214292 10625 0 1336796 1036 654596 313024 11
surface-2cam-1k 1000 0 0 0 2 2 2000 0 0 0 0 0 0 0 2 0 0 0 0 0 0 188833 203083 238041 10791 0 1336320 1033 654400 312928 11
```

### GPU record

```
render_scale_gpu adapter=Apple M5 backend=Metal viewport=1280x720 format=Rgba8Unorm
label encode_median_ns encode_p95_ns completion_median_ns completion_p95_ns first_encode_ns draw_calls world_draw_calls clear_draw_calls camera_passes offscreen_passes sprite_instances mesh_instances submitted_instances instance_capacity_bytes steady_buffer_grows texture_creates render_target_creates base_upload_bytes mesh_cache alloc_bytes alloc_count peak_extra_bytes samples
sprite-opaque-1k 48250 77333 222500 2783417 126375 1 1 0 1 0 1000 0 1000 155648 0 3 0 72 0 121637 56 112732 11
sprite-opaque-10k 159125 214000 352958 4562292 254667 1 1 0 1 0 10000 0 10000 2121728 0 3 0 72 0 1129637 56 1120732 11
sprite-opaque-100k 1398666 1659000 1584625 4405250 2048667 1 1 0 1 0 100000 0 100000 16801792 0 3 0 72 0 11209637 56 11200732 7
sprite-mask-10k 157708 195708 325958 3852875 221208 1 1 0 1 0 10000 0 10000 2121728 0 3 0 72 0 1129637 56 1120732 11
sprite-blend-1k 43917 51208 194292 220708 88958 1 1 0 1 0 1000 0 1000 155648 0 3 0 72 0 121637 56 112732 11
sprite-blend-10k 151250 167584 295291 4656208 192000 1 1 0 1 0 10000 0 10000 2121728 0 3 0 72 0 1129637 56 1120732 11
sprite-blend-100k 1377125 1697333 2688541 4696250 1817500 1 1 0 1 0 100000 0 100000 16801792 0 3 0 72 0 11209637 56 11200732 7
sprite-opaque-materials-10k 173792 192250 277584 2811750 241125 8 8 0 1 0 10000 0 10000 2121728 0 10 0 520 0 1132741 60 1120732 11
sprite-blend-materials-1k 280875 309583 231166 3253875 628500 1000 1000 0 1 0 1000 0 1000 155648 0 10 0 520 0 596037 73 241134 11
sprite-orient-10k 154875 187042 260458 3541375 224250 1 1 0 1 0 10000 0 10000 2121728 0 3 0 72 0 1129861 56 1120732 11
sprite-coplanar-10k 146584 157625 253084 270417 226708 1 1 0 1 0 10000 0 10000 2121728 0 3 0 72 0 1129861 56 1120732 11
sprite-dense-partial-10k 54041 83000 195500 2134625 73334 1 1 0 1 0 1800 0 1800 286720 0 3 0 72 0 211461 56 202332 11
sprite-sparse-tight-10k 29875 39875 197000 275875 48583 1 1 0 1 0 20 0 20 28672 0 3 0 72 0 12101 56 6239 11
sprite-opaque-10k-unlit 149583 188333 255291 2554542 218667 1 1 0 1 0 10000 0 10000 2121728 0 3 0 72 0 1129861 56 1120732 11
sprite-lights-16-10k 151416 196667 395583 3730291 189125 1 1 0 1 0 10000 0 10000 2121728 0 3 0 72 0 1129861 56 1120732 11
sprite-cam-7-10k 1208750 1711833 294417 3656834 1563666 13 7 6 7 0 70000 0 70000 2121728 0 3 0 72 0 7914019 392 1130338 11
sprite-cam-cycle-7-10k 1193958 1329917 243625 3557791 1316916 13 7 6 7 0 70000 0 70000 2121728 0 3 0 72 0 7914019 392 1130338 11
mesh-shared-100 34542 38166 179875 222875 400750 1 1 0 1 0 0 100 100 40960 0 3 0 72 1 20405 58 15863 11
mesh-shared-1k 90167 127042 216708 2601750 156334 1 1 0 1 0 0 1000 1000 155648 0 3 0 72 1 106805 58 102263 11
mesh-shared-10k 591667 656916 297333 3463291 667833 1 1 0 1 0 0 10000 10000 1073152 0 3 0 72 1 970805 58 966263 11
mesh-distinct-1k 91833 103083 211417 228708 184667 8 8 0 1 0 0 1000 1000 155648 0 3 0 72 8 108765 59 102431 11
mesh-materials-1k 94042 105667 208917 2031500 187708 8 8 0 1 0 0 1000 1000 155648 0 10 0 520 1 112541 62 103470 11
mesh-sphere-100 35375 45458 193083 289083 60583 1 1 0 1 0 0 100 100 40960 0 3 0 72 1 20573 58 16031 11
mesh-lights-16-1k 87666 92458 231917 242375 129666 1 1 0 1 0 0 1000 1000 155648 0 3 0 72 1 106973 58 102431 11
lane-mixed 78459 135458 276167 2059958 163917 5 5 0 1 0 2536 22 2558 319488 0 4 0 136 1 300893 78 258860 11
lane-cameras-7 615250 750833 431000 3660125 717292 41 35 6 7 0 17713 154 17867 319488 0 4 0 136 1 2101883 546 281680 11
lane-lights-16 77250 111042 458542 3075166 143042 5 5 0 1 0 2536 22 2558 319488 0 4 0 136 1 300893 78 258860 11
offscreen-1k 87167 100500 222292 316167 154167 4 4 0 2 1 2002 0 2002 155648 0 3 1 72 0 246110 114 117783 11
surface-2cam-1k 89834 108375 235542 2829292 186792 3 2 1 2 0 2000 0 2000 155648 0 3 0 72 0 244938 112 117671 11
```

## Depth view

Recorded 2026-10-24 in the afternoon, America/Chicago, on `feat/m6-depth-view` at the implementation that became `4bd591d`. The machine is the same Apple M5, Metal, macOS arm64, rustc 1.99.0 class as the render-scale run above. This section does not replace that table.

| | |
|---|---|
| Command | `cargo run --release -p engine-renderer-wgpu --example depth_view_bench` |
| Scene | 64 cutout sprites and 4 shared cuboids, 1280×720, orthographic, clip range `0.5..40`. Not the render-scale catalog |
| Cameras | 1, or 7 side-by-side viewports of the same scene |
| Samples | 2 warmup, then 11. Median is the middle sample. p95 is nearest-rank |
| `encode_ns` | `SceneRenderer::render` after the previous sample's completion wait. Command recording and submit. No readback |
| `completion_ns` | The following `device.poll(wait_indefinitely)`. GPU work plus driver synchronization. Not a pass timestamp. `timestamp_writes` stays `None` |

`bind_group_creates` is cumulative on one renderer. The first shaded frame creates 3. The first depth frame creates the sample group and the counter becomes 4. Later rows stay at 4.

Draws for one camera: 2 world draws shaded, 3 with the depth pass (`depth_view_draws` 1). Seven cameras: 14 world draws in both modes. Shaded is 20 draws because the six later cameras each clear their viewport. Depth is 27 draws: those 20 plus one depth pass per camera. World draws do not change.

The depth image is one `Depth32Float`, `1280 * 720 * 4 = 3,686,400` bytes, created for shaded frames as well (`RENDER_ATTACHMENT | TEXTURE_BINDING`). There is no second copy. The uniform is 128 bytes, created with the renderer. The pipeline is created at init and is not bound while every camera is shaded. A shaded frame does not run the pass and does not create the sample group. The Metal heap size was not queried.

Two release runs, one after the other, same binary:

| Run | Mode | Cameras | encode median ns | encode p95 ns | completion median ns | completion p95 ns |
|---|---|---|---|---|---|---|
| 1 | shaded | 1 | 39,000 | 57,625 | 214,375 | 2,074,416 |
| 1 | depth | 1 | 33,958 | 35,917 | 200,834 | 900,958 |
| 1 | shaded | 7 | 195,458 | 273,750 | 359,584 | 2,972,208 |
| 1 | depth | 7 | 251,458 | 495,250 | 491,167 | 1,891,292 |
| 2 | shaded | 1 | 68,417 | 88,875 | 412,500 | 1,735,625 |
| 2 | depth | 1 | 82,167 | 112,750 | 311,292 | 4,831,834 |
| 2 | shaded | 7 | 533,209 | 890,083 | 458,875 | 3,163,875 |
| 2 | depth | 7 | 513,458 | 810,083 | 690,500 | 3,273,167 |

Draw counts and the bind-group counter matched on both runs. Encode time did not. In run 1 the one-camera depth encode was faster than shaded. In run 2 it was slower. Seven cameras swapped the same way. The difference is inside the run-to-run spread of this scene, so this checkpoint does not assign the extra pass a microsecond cost. The pass is also not free: it is one extra draw, one stored depth attachment, one sample bind group on first use, and one 128-byte uniform write per visualizing camera. GPU execution time inside the pass remains unmeasured. The completion-wait p95 is not a frame budget.

## Camera presets

Recorded 2026-10-24 with the public constructors (`16ed495`). `PlacedCamera` and `CameraProjection::oblique_orthographic` / `oblique_perspective` build a `Camera`, a `Transform`, or a `Custom` matrix before extract. Extraction, batching, and the wgpu backend are the same path as a hand-built camera. This checkpoint did not change that work, so it records no new timing run and does not reuse the render-scale table as a preset benchmark.

The release screenshots that compare against the historical baselines kept their draw counts: `basic_2_5d` 10 draws, `basic_2_5d --ortho` 10 draws, `spatial_scene --camera perspective` 10 draws, and `spatial_scene --camera orthographic` 8 draws. `depth_view_draws` stayed 0. A GPU test of `PlacedCamera::perspective` against `Camera::perspective` with the same transform and clip matched pixels and draw counts.

## Orthographic pixel snap

Recorded 2026-10-24 in the evening, America/Chicago, on `feat/m6-d06a-pixel-snap` at `4a87bb0`, before the 6:44 PM check. The machine is Apple M5, Metal, macOS arm64, `rustc 1.99.0`. This section does not replace the render-scale table above. The two snap rows are comparison cases (`gpu: false`). They are not part of that historical TSV.

| | |
|---|---|
| Command | `DIMORA_SCALE_CASES=sprite-opaque-10k,sprite-opaque-10k-snap,sprite-cam-2-10k,sprite-cam-2-10k-snap cargo run --release -p engine-renderer --bin render_scale_bench` |
| Scene | The existing 10,000 opaque-sprite world, viewport 1280×720, one orthographic camera, or the same sprites under two cameras |
| Snap | The `-snap` rows set `pixel_snap` on those cameras. The plain rows leave the default `false` |
| Samples | 4 warmup, then 11. Median is the middle sample. p95 is the slowest sample |
| What is timed | `extract_scene` plus dropping the `RenderScene`. Not a GPU timestamp. `timestamp_writes` stays `None` |

`alloc_bytes` counts allocator requests, including a realloc at its new size, and does not subtract frees. It is not resident memory and not GPU memory. Draw and batch counts are the extract stats: one sprite batch and one pass for 10,000 sprites, two batches and two passes for two cameras. Snap does not add a pass, a batch, or a draw.

Two release runs, the second on the binary the first had just built. Allocation bytes, allocation count, visible sprites, batches, and passes matched inside each pair on both runs.

| Run | Row | extract median ns | extract p95 ns | first extract ns | alloc_bytes | alloc_count | sprite_batches | passes |
|---|---|---|---|---|---|---|---|---|
| 1 | sprite-opaque-10k | 2,076,583 | 2,363,792 | 2,919,792 | 12,850,736 | 10,030 | 1 | 1 |
| 1 | sprite-opaque-10k-snap | 1,512,750 | 1,589,875 | 1,715,750 | 12,850,736 | 10,030 | 1 | 1 |
| 1 | sprite-cam-2-10k | 1,994,167 | 2,044,000 | 2,151,584 | 19,351,680 | 10,045 | 2 | 2 |
| 1 | sprite-cam-2-10k-snap | 2,091,166 | 2,113,375 | 2,091,875 | 19,351,680 | 10,045 | 2 | 2 |
| 2 | sprite-opaque-10k | 2,157,042 | 2,749,416 | 3,337,416 | 12,850,736 | 10,030 | 1 | 1 |
| 2 | sprite-opaque-10k-snap | 1,637,334 | 1,674,833 | 1,876,375 | 12,850,736 | 10,030 | 1 | 1 |
| 2 | sprite-cam-2-10k | 1,965,250 | 2,045,875 | 2,280,042 | 19,351,680 | 10,045 | 2 | 2 |
| 2 | sprite-cam-2-10k-snap | 2,110,000 | 2,178,458 | 2,179,375 | 19,351,680 | 10,045 | 2 | 2 |

The one-camera snap row ran second and was faster both times (about 1.51 ms and 1.64 ms, against 2.08 ms and 2.16 ms). That matches the catalog's known first-row effect and is not a snap speedup. The two-camera rows ran after that warmup. Snap was slower by about 97 µs on run 1 and about 145 µs on run 2, on a median near 2.0 ms. A few percent, one machine, two runs. This checkpoint does not assign snapping a CPU budget. The added work is a few vector operations inside `CameraView::new`, once per orthographic camera. GPU execution time was not measured. No new pipeline, texture, or render pass exists for the flag.

### Order-controlled comparison

Recorded 2026-10-25 in the afternoon, America/Chicago, on the same Apple M5 and the same release binary of `4a87bb0`. One orthographic camera, 10,000 opaque sprites, viewport 1280×720. The case list was `sprite-opaque-10k`, `sprite-opaque-10k-snap`, `sprite-opaque-10k-snap`, `sprite-opaque-10k`, then that sequence again. 4 warmup and 11 samples inside each row. `alloc_bytes` stayed 12,850,736 and `alloc_count` stayed 10,030. Visible sprites stayed 10,000. Sprite batches stayed 1 and passes stayed 1.

| Sequence | Order | extract median ns | extract p95 ns |
|---|---|---|---|
| 1 | off | 1,644,583 | 2,205,125 |
| 1 | on | 1,593,166 | 1,976,166 |
| 1 | on | 1,660,417 | 2,994,958 |
| 1 | off | 1,796,917 | 1,870,375 |
| 2 | off | 1,894,083 | 3,146,000 |
| 2 | on | 1,871,417 | 2,680,875 |
| 2 | on | 2,282,125 | 2,512,125 |
| 2 | off | 2,339,083 | 3,111,375 |

The medians do not move in one direction. In the first sequence the first snap row is about 51 µs under the opening unsnapped row, and the closing unsnapped row is the slowest. In the second sequence the rows rise from about 1.89 ms to about 2.34 ms across both settings. Several p95 values sit hundreds of microseconds above their medians. The earlier two-camera gap of about 0.1 ms is inside this spread. No extract-time improvement or regression is claimed. No renderer change follows from these numbers. GPU pass time remains unmeasured.

## Integer-resolution presentation

Recorded 2026-10-25 at about 3:05 PM America/Chicago, on `feat/m6-d06b-internal-resolution` at `d491bda`, before the 3:08 PM check. The machine is Apple M5, Metal, macOS arm64, `rustc 1.99.0`. This section does not replace the render-scale table or the pixel-snap table. The scene is the depth-view bench scene, not the 10,000-sprite catalog.

| | |
|---|---|
| Command | `cargo run --release -p engine-renderer-wgpu --example internal_resolution_bench` |
| Scene | 64 cutout sprites and 4 shared cuboids, viewport 1280×720, one orthographic camera or two split viewports |
| Internal | The internal rows set 320×180. The direct rows leave `internal_resolution` unset |
| Samples | 4 warmup, then 11, and the whole table twice. Median is the middle sample. p95 is the slowest sample |
| Extract | `extract_scene`. Not a GPU timestamp |
| Encode | `SceneRenderer::render` after the previous completion wait. Records and submits. Does not map a readback |
| Completion | The following `device.poll(wait_indefinitely)`. GPU execution and driver synchronization together. Not a timestamp query in the scene pass or the blit |

One 320×180 color image is 230,400 bytes (`width * height * 4`). The matching `Depth32Float` image is the same number of bytes. Two cameras retain both pairs: 460,800 color bytes and 460,800 depth bytes. A direct row retains neither.

World draws stay 2 for one camera and 4 for two. Presentation draws are 0, 1, or 2 with the camera count on internal rows, and 0 on every direct row. Direct rows create no private image. Internal rows create one image per camera during warmup and do not create or reallocate again across the samples.

| Run | Mode | Cameras | extract median ns | extract p95 ns | encode median ns | encode p95 ns | completion median ns | completion p95 ns |
|---|---|---|---|---|---|---|---|---|
| 1 | direct | 1 | 9,750 | 17,000 | 31,167 | 40,000 | 371,208 | 400,833 |
| 1 | internal | 1 | 7,125 | 8,333 | 36,916 | 41,291 | 426,250 | 446,667 |
| 1 | direct | 2 | 9,709 | 11,042 | 53,916 | 106,000 | 430,375 | 470,959 |
| 1 | internal | 2 | 9,625 | 10,792 | 73,416 | 79,958 | 430,208 | 694,833 |
| 2 | direct | 1 | 7,042 | 7,250 | 29,125 | 33,875 | 371,625 | 461,583 |
| 2 | internal | 1 | 7,041 | 7,208 | 34,708 | 45,542 | 409,708 | 429,083 |
| 2 | direct | 2 | 9,542 | 9,833 | 53,459 | 65,208 | 436,875 | 530,166 |
| 2 | internal | 2 | 9,750 | 10,208 | 73,625 | 84,666 | 425,792 | 510,750 |

Extract of this small scene is about 7–10 µs. Run 1's one-camera internal row was faster than the direct row that preceded it, which is the first-row effect. Run 2's one-camera medians match (7,042 ns and 7,041 ns). The two-camera extract rows stay within a few hundred nanoseconds. No extract budget is assigned.

Encode medians for one internal camera are about 6 µs above the direct row (36.9 µs and 34.7 µs, against 31.2 µs and 29.1 µs). Two internal cameras are about 20 µs above the direct pair (73.4 µs and 73.6 µs, against 53.9 µs and 53.5 µs). That is CPU time recording the extra blit, on a scene whose encode is a few tens of microseconds. It is not a GPU pass time. Completion waits overlap between modes, from about 0.37 ms to about 0.53 ms at the median, and the p95 moves by more than the encode gap. No GPU cost is claimed.

### Size change and disable

One renderer, after the timed rows. Counters are cumulative on that renderer.

| Step | creates | reallocs | drops | cameras | color bytes | depth bytes | world draws | presentation draws |
|---|---|---|---|---|---|---|---|---|
| 320×180 | 1 | 0 | 0 | 1 | 230,400 | 230,400 | 2 | 1 |
| 320×180 again | 1 | 0 | 0 | 1 | 230,400 | 230,400 | 2 | 1 |
| 160×90 | 1 | 1 | 0 | 1 | 57,600 | 57,600 | 2 | 1 |
| direct | 1 | 1 | 1 | 0 | 0 | 0 | 2 | 0 |

The second 320×180 frame reuses the images. The size change reallocates once and retains the new size only. Disabling the feature drops the private images and adds no presentation draw. World draws stay 2. A disabled camera therefore adds no private image and no presentation draw on this path.

Acceptance on 2026-10-25 did not change fit, batching, or the blit. The table was not re-run. GPU tests in `internal_resolution.rs` cover index reuse, sixteen create-and-remove cycles, reorder, and a move onto an offscreen target. Those tests count creates, drops, and bytes. They are not timings. GPU pass timestamps are still unmeasured.

## Optional 4× MSAA

Recorded 2026-10-26 at 11:39 AM America/Chicago, on `feat/m6-d06c-msaa`, Apple M5, Metal, macOS arm64. This section does not replace the integer-resolution table. The scene is one direct orthographic camera, 64 cutout sprites, and 4 shared cuboids, at 1280×720. [ADR-0024](adr/0024-optional-4x-msaa.md) is settled for the 1×/4× contract. These rows are the measured cost, not a GPU pass time. D-06 and M6 stay open.

| | |
|---|---|
| Command | `cargo run --release -p engine-renderer-wgpu --example msaa_bench` |
| Scene | 64 cutout sprites and 4 cuboids, one direct orthographic camera, viewport 1280×720 |
| Samples | 4 warmup, then 11, and the pair of modes twice. Median is the middle sample. p95 is the slowest sample |
| Encode | `SceneRenderer::render` after the previous completion wait. Records and submits. Does not map a readback |
| Completion | The following `device.poll(wait_indefinitely)`. GPU execution and driver synchronization together. Not a timestamp query in the scene pass or the resolve |
| Bytes | `width * height * 4 * 4` for 4× color and the same for 4× depth. A size calculation, not a driver allocation measurement |

`set_msaa` runs before the timed samples, so 4× pipeline creation is outside the loop. The first render before warmup creates the images. A steady frame does not create or reallocate them. The one-sample rows retain no 4× image. While 4× is selected the one-sample surface depth is not also kept. One sample of 1280×720 color or depth is 3,686,400 bytes. Each 4× image is 14,745,600 bytes. The pair is 29,491,200 bytes, calculated.

World draws stay 2. Resolve draws are 0 at one sample and 1 at four. Pipeline creates are 0 and 1. Image creates are 0 and 1.

| Run | Mode | encode median ns | encode p95 ns | completion median ns | completion p95 ns | world draws | resolve draws | pipeline creates | image creates | color bytes | depth bytes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 1× | 50,500 | 64,375 | 472,708 | 619,125 | 2 | 0 | 0 | 0 | 0 | 0 |
| 1 | 4× | 56,000 | 73,541 | 680,291 | 1,045,834 | 2 | 1 | 1 | 1 | 14,745,600 | 14,745,600 |
| 2 | 1× | 31,209 | 63,791 | 372,958 | 2,407,917 | 2 | 0 | 0 | 0 | 0 | 0 |
| 2 | 4× | 33,500 | 78,750 | 329,333 | 413,834 | 2 | 1 | 1 | 1 | 14,745,600 | 14,745,600 |

Encode medians at 4× are about 5.5 µs above one sample on run 1 and about 2.3 µs above it on run 2, on an encode of a few tens of microseconds. That is CPU time recording the extra resolve. It is not a GPU pass time. Completion medians overlap. Run 2's 4× median is lower than its one-sample median, and run 2's one-sample p95 is an outlier above 2 ms. No GPU cost is claimed. GPU pass timestamps were not measured.

The same direct scene was measured again on 2026-10-26 at 1:08 PM America/Chicago, after stage 2, still with a `RENDER_ATTACHMENT` color target and no `Load` sampling. Run 1: one sample encode median 41,083 ns, p95 48,250, completion median 365,000, p95 767,166. Four samples: 36,125, 91,292, 479,542, 548,917. Run 2: one sample 28,542, 31,375, 298,917, 319,042. Four samples: 34,292, 40,791, 325,292, 806,708. World draws stay 2. Resolve draws stay 0 and 1. Pipeline creates stay 0 and 1. Image creates stay 0 and 1. Calculated bytes stay 0 and 14,745,600 for each 4× image. Run 1's 4× encode median is below its one-sample median. The difference is a few microseconds and is not a GPU cost.

### Stage 2 workloads

Same command, machine, warmup, and sample count, at 1:08 PM America/Chicago. `extract_ns` is `extract_scene` only and does not depend on the sample count. `timed_image_creates` is the image-create counter during the 11 timed frames, after warmup. It is 0 on every row. Lifetime `image_creates` is 1 at 4× because the frame before warmup allocates. Pipeline creation is outside the timed loop.

Sprite-heavy is 256 cutout sprites and no meshes. Mesh-heavy is 64 shared cuboids and no sprites. Mixed is the 64-sprite, 4-cuboid scene above. Split is that scene in two side-by-side viewports. Offscreen writes it to a 1280×720 target. Internal presents it from 320×180. Load adds a second full-viewport `CameraClear::Load` camera; that row's color target also has `TEXTURE_BINDING`. Depth presents the mixed scene. World draws of 1 mean the sprites or the cuboids batched. Bytes are calculated. The internal pair is 320 × 180 × 16 = 921,600 bytes per image, not the 1280×720 surface pair.

| Workload | Run | Mode | extract median ns | encode median ns | encode p95 ns | completion median ns | completion p95 ns | world | resolve | expand | depth | present | color bytes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| sprites | 2 | 1× | 26,125 | 31,000 | 37,917 | 187,084 | 200,167 | 1 | 0 | 0 | 0 | 0 | 0 |
| sprites | 2 | 4× | 26,125 | 38,292 | 51,792 | 258,042 | 1,623,083 | 1 | 1 | 0 | 0 | 0 | 14,745,600 |
| meshes | 2 | 1× | 3,250 | 25,917 | 48,291 | 209,834 | 1,254,542 | 1 | 0 | 0 | 0 | 0 | 0 |
| meshes | 2 | 4× | 3,250 | 33,250 | 37,334 | 239,625 | 340,125 | 1 | 1 | 0 | 0 | 0 | 14,745,600 |
| mixed | 2 | 1× | 6,916 | 26,584 | 26,959 | 207,750 | 324,875 | 2 | 0 | 0 | 0 | 0 | 0 |
| mixed | 2 | 4× | 6,916 | 32,917 | 35,292 | 234,125 | 370,916 | 2 | 1 | 0 | 0 | 0 | 14,745,600 |
| split | 2 | 1× | 9,542 | 50,166 | 56,875 | 236,625 | 319,292 | 4 | 0 | 0 | 0 | 0 | 0 |
| split | 2 | 4× | 9,542 | 62,833 | 71,875 | 294,875 | 1,759,125 | 4 | 2 | 0 | 0 | 0 | 14,745,600 |
| offscreen | 2 | 1× | 7,000 | 26,167 | 51,625 | 224,334 | 1,223,750 | 2 | 0 | 0 | 0 | 0 | 0 |
| offscreen | 2 | 4× | 7,000 | 32,125 | 35,000 | 240,625 | 299,416 | 2 | 1 | 0 | 0 | 0 | 14,745,600 |
| internal | 2 | 1× | 6,917 | 33,334 | 34,209 | 210,875 | 259,458 | 2 | 0 | 0 | 0 | 1 | 0 |
| internal | 2 | 4× | 6,917 | 42,250 | 51,292 | 235,959 | 355,583 | 2 | 1 | 0 | 0 | 1 | 921,600 |
| load | 2 | 1× | 9,625 | 47,208 | 53,708 | 229,583 | 273,667 | 4 | 0 | 0 | 0 | 0 | 0 |
| load | 2 | 4× | 9,625 | 74,959 | 80,000 | 306,667 | 1,819,875 | 4 | 2 | 1 | 0 | 0 | 14,745,600 |
| depth | 2 | 1× | 7,084 | 38,042 | 109,500 | 215,375 | 300,125 | 2 | 0 | 0 | 1 | 0 | 0 |
| depth | 2 | 4× | 7,084 | 39,000 | 44,375 | 238,209 | 458,666 | 2 | 0 | 0 | 1 | 0 | 14,745,600 |

Run 1 is the same shape. Its 4× encode medians are 39,417 (sprites), 33,375 (meshes), 32,583 (mixed), 64,250 (split), 34,542 (offscreen), 40,583 (internal), 74,125 (load), and 36,833 (depth). Draw counts and byte counts match run 2. Completion p95 moves by more than a millisecond on several rows. That wait is queue completion, not a timestamp query. No GPU cost is claimed.

The load row's extra encode, about 28 µs on run 2, is the CPU recording the expansion pass plus the second scene pass. Extraction does not change with the sample count. Switching sample counts on one renderer created the pipelines once, allocated one surface pair, allocated nothing on the next steady frame, reallocated once when the surface became 640×360, dropped that pair on the return to one sample (`image_count` 0), and created the 1280×720 pair again without a second pipeline. Restored calculated bytes were 14,745,600 for color and the same for depth.

### Window composition

Recorded 2026-10-26 at 2:26 PM America/Chicago, same command, machine, viewport, warmup, and sample count, after 4× window output moved to a private image. The direct-scene and stage 2 tables above were produced again in the same process and are not replaced. Their draw counts and calculated 4× bytes match the 1:08 PM rows. Run 2 encode medians on that direct path were higher: sprites 42,584 ns, meshes 36,250, mixed 35,208, split 251,583, offscreen 39,500, internal 45,042, load 162,833, depth 104,084. Split run 1 encode p95 was 2,733,792 ns and load run 2 encode p95 was 2,021,042 ns. Those waits are CPU samples with outliers. They are not a GPU pass time and they are not a claim that the direct path got slower by a stable amount.

The presented rows call `present_to_surface` on the 64-sprite, 4-cuboid scene. The destination usage is `RENDER_ATTACHMENT` only. Encode is still CPU recording and submit. Completion is still `device.poll(wait_indefinitely)`, not a timestamp query. `surface_frame_bytes` is `width * height * 4`, a size calculation. At 1280×720 that is 3,686,400 bytes, one sample, in addition to the 4× color image. The printed `msaa_color_bytes` column is the 4× color calculation and does not include 4× depth. That depth image remains 14,745,600 bytes on the direct 4× rows. `steady_frame_creates` is the create counter during the 11 timed frames.

| Run | Mode | encode median ns | encode p95 ns | completion median ns | completion p95 ns | world | resolve | expand | frame present | frame bytes | 4× color bytes | frame creates | steady creates | reallocs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 1× | 77,917 | 121,875 | 324,917 | 361,708 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 1 | 4× | 147,167 | 309,125 | 533,209 | 600,125 | 2 | 1 | 0 | 1 | 3,686,400 | 14,745,600 | 1 | 0 | 0 |
| 2 | 1× | 99,542 | 192,625 | 222,541 | 243,500 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 2 | 4× | 163,208 | 253,208 | 721,791 | 1,547,708 | 2 | 1 | 0 | 1 | 3,686,400 | 14,745,600 | 1 | 0 | 0 |

One sample allocates no private frame and records no frame-present draw. Four samples add one present draw. Timed frames do not create another frame image. After the timed 4× samples, presenting at 640×360 reallocates the private image once (`reallocs` 1) and returning to one sample drops that image and the 4× images (`dropped_frame_and_msaa` true, `frame_drops` 1). Run 2's 4× encode median is about 64 µs above its one-sample presented median. That is CPU time recording the resolve and the copy. It is not a GPU cost. The one-sample presented encode is the same scene call path as a direct render plus a drop of an image that was never created. Its median sits inside the spread of the direct one-sample encodes from this run. No separate 1× tax is claimed.

