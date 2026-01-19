# MCP Server API Improvements

This document summarizes recommended API additions and improvements for the VCD / MCP (waveform) server based on real usage during waveform analysis and debugging. The goal is to make common analysis tasks (edge/pulse counts, summaries, chunked retrieval) fast, deterministic, and network-efficient.

## Executive summary ✅
- Problem: fetching long value streams for frequently-changing signals produces very large payloads and often truncation; clients need small, fast answers for common queries (counts, summaries, change counts).
- Quick wins (high priority):
  - File/simulation metadata endpoint
  - Signal summary endpoint
  - Edge/pulse counting endpoint
- Longer-term: chunked streaming/pagination, value filters, async jobs for heavy aggregations, and convenient derived metrics.

---

## Prioritized list of recommended endpoints 🔧

1. **File / simulation metadata** (GET /files/{file}/info)
   - Returns: `{ timescale, first_time, last_time, signal_count, top_scopes, file_size_bytes }`
   - Rationale: help clients avoid guessing safe time windows and choose chunk sizes.

2. **Signal summary / stats** (GET /signals/summary)
   - Query params: `file`, `signals=[...]`, `start`, `end`
   - Returns per-signal: `last_value`, `change_count`, `first_change_time`, `last_change_time`, `bit_width`, `min`, `max`, `mean` (when numeric)
   - Rationale: small payloads that answer most exploratory questions quickly.

3. **Edge / pulse counting** (GET /signals/count_edges or POST /signals/count_edges)
   - Params: `file`, `signal`, `edge=rising|falling|both`, `bit_index` (for vectors), `start`, `end`
   - Returns: `{ count: <int>, details?: { per_bit: [...] } }`
   - Rationale: directly supports counts such as clock rising edges or instruction completion pulses (our most common need).

4. **Value stream pagination / streaming** (GET /signals/values)
   - Params: `file`, `signals=[...]`, `start`, `end`, `limit`, `cursor`, `step` (optional), `only_changes=true|false`
   - Support HTTP chunked streaming or WebSocket subscription for large streams.
   - Rationale: avoids single huge responses and supports robust client-side aggregation.

5. **Value filters / change-only views** (enhancement to /signals/values)
   - Params: `only_changes=true`, `only_bit=<n>`, `mask=<hex>` or `match=<value>`
   - Rationale: wide vectors often only need one bit or the change-only view.

6. **Value-at-time** (GET /signals/value_at)
   - Params: `file`, `signal`, `time`
   - Returns last value at or before requested time.
   - Rationale: cheap check for boundary conditions.

7. **Compression & alternative response formats**
   - Support `Accept-Encoding: gzip` and `format=jsonl|csv|binary`.
   - Rationale: reduces network payload, easier to parse or stream.

8. **Async large jobs** (POST /jobs -> GET /jobs/{id})
   - For heavy aggregations (e.g., multi-signal correlational metrics over long times), return job id and let clients poll for results.
   - Rationale: avoids request timeouts and makes long work reliable.

9. **Wildcard / multi-signal queries**
   - Allow `signals=TOP.top.*` or similar patterns.
   - Rationale: reduces round-trips when operating on groups of signals.

10. **Derived / convenience metrics** (optional)
    - Pre-baked metrics such as `GET /metrics/instruction_count?file=...` which internally uses `instr_complete` or registered counters.
    - Rationale: simplifies domain-specific common tasks.

---

## Example request/response schemas ✏️

### File info

Request:
```
GET /files/out.vcd/info
```
Response:
```json
{
  "file": "out.vcd",
  "timescale": "1 PS",
  "first_time": 0,
  "last_time": 10005,
  "signal_count": 173,
  "top_scopes": ["TOP"],
  "file_size_bytes": 4100000
}
```

### Count edges

Request:
```
GET /signals/count_edges?file=out.vcd&signal=TOP.clk&edge=rising&start=0&end=10005
```
Response:
```json
{ "signal": "TOP.clk", "edge": "rising", "start": 0, "end": 10005, "count": 5002 }
```

### Signals summary

Request:
```
GET /signals/summary?file=out.vcd&signals=TOP.top.instr_complete,TOP.top.completed_instr_reg&start=0&end=10005
```
Response (abridged):
```json
{
  "TOP.top.instr_complete": { "change_count": 5003, "first_change_time": 8, "last_change_time": 10005, "last_value": 1, "bit_width": 1 },
  "TOP.top.completed_instr_reg": { "change_count": 2500, "bit_width": 32 }
}
```

---

## Implementation & backward-compatibility notes 🧭
- Make these additive: add new endpoints or optional query parameters without changing existing endpoints.
- Return small summaries by default; require explicit flags (e.g., `full=true`) for full streams.
- Implement server-side counters where possible (e.g., counting edges) to avoid transferring large datasets.
- For heavy jobs (scan entire file), implement async jobs (POST -> 202 + job id) to avoid timeouts.
- Document rate limits / pagination to avoid accidental large dumps.

---

## Prioritization & suggested roadmap 🛣️
1. Quick: `GET /files/{file}/info` + `GET /signals/summary` + `GET /signals/count_edges` (addresses most pain points).  
2. Medium: `GET /signals/values` pagination + `only_changes` flag + compression support.  
3. Longer: Async job system + wildcard queries + derived metrics registration.

---

## Small implementation sketch for `count_edges` (algorithm)
- Single-bit signals: count transitions where previous==0 and current==1 for rising.
- Vector signals: take `bit_index` or `mask` param; count per-bit transitions.
- Optimize: scan VCD change records; maintain last seen value per signal (or per-bit) and increment.
- Provide `start` and `end` to restrict scan.

---

## Notes from use-case that motivated this
- During interactive analysis of `out.vcd` (println_test), fetching the entire value stream for frequently-changing signals produced multi-megabyte payloads and truncation. A server-side rising-edge count and lightweight summary would have returned the metrics instantly.

---

## Next steps
- Implement the three quick-win endpoints and add tests.  
- Add API docs and short examples in the server's README or Swagger/OpenAPI spec.  
- Optional: implement a simple job queue for heavy tasks.

---

If you'd like, I can open a draft PR that adds these endpoints (server + tests + OpenAPI entries) starting with `count_edges`, `summary` and `file info`.
