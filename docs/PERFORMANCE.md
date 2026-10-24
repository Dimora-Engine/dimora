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

A camera that sees every chunk still reads every cell. The win is rejecting a chunk before that read. Occupancy of a rejected chunk is a stored count, refreshed only when `set` or `invalidate` marks the chunk dirty. Sprite instances are not cached. Each visible tile still builds a world matrix, because the map transform can change every frame.

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
