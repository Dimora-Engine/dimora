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

A camera that sees the whole map pays for every occupied tile. One orthographic camera on a dense 1000×1000 map takes a median of 37.1 ms and builds one million instances in one batch, asking the allocator for about 395 MB. Seven such cameras take 234 ms orthographic and 295 ms perspective, build seven million instances in seven batches, and ask for about 2.76 GB. The seven-camera cost is several times one camera because each camera repeats the grid walk and owns its own instances.

A partial view still walks the whole grid. The orthographic partial camera sees about 27 percent of a dense 1000×1000 map and takes 18.0 ms, about half of the full view, not 27 percent of it. Culled tiles still pay for a matrix and a sphere test. Perspective partial sees about 49 percent and takes 21.8 ms against a 31.2 ms full view.

Empty cells are cheap relative to instance building. A fully visible sparse 1000×1000 map (125,000 occupied tiles) takes 4.3 ms orthographic with one camera, against 37.1 ms dense. The walk is still one million cells.

Batches stay at one per camera in every row. Instance count equals `visible`. Simulation time is not in the extract column.

Chunking is not in these numbers. A later run of the same binary is the comparison.

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
