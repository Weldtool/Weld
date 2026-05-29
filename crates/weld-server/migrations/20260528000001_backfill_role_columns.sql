-- Backfill review_column_id / in_progress_column_id / done_column_id on
-- projects created before these mapping fields existed. New projects already
-- get these set in services/project/create.rs; this only fills the gap for
-- pre-existing projects whose default columns still match the seeded names.
UPDATE projects p SET review_column_id = c.id
FROM columns c
WHERE c.project_id = p.id
  AND p.review_column_id IS NULL
  AND lower(c.name) = 'review';

UPDATE projects p SET in_progress_column_id = c.id
FROM columns c
WHERE c.project_id = p.id
  AND p.in_progress_column_id IS NULL
  AND lower(c.name) = 'in progress';

UPDATE projects p SET done_column_id = c.id
FROM columns c
WHERE c.project_id = p.id
  AND p.done_column_id IS NULL
  AND lower(c.name) IN ('completed', 'done');
