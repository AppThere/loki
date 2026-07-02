-- SPDX-License-Identifier: Apache-2.0
-- Snapshot sequence guard (ADR-C013 compaction). A snapshot covers every
-- oplog entry with seq <= snapshot_seq; the pointer only ever moves forward
-- (enforced by the conditional UPDATE in the store), so a slow concurrent
-- compactor can never regress it and orphan truncated updates.

ALTER TABLE doc_meta ADD COLUMN snapshot_seq BIGINT NOT NULL DEFAULT 0;
