-- New York City: the five boroughs.
-- Overture release 2026-09-23.1 (schema 2.0.0).
INSTALL httpfs; LOAD httpfs;
SET s3_region='us-west-2';
SET enable_progress_bar=false;

CREATE OR REPLACE MACRO in_nyc(b) AS
  b.xmin <= -73.70 AND b.xmax >= -74.26 AND b.ymin <= 40.92 AND b.ymax >= 40.49;

COPY (
  SELECT * FROM read_parquet('s3://overturemaps-us-west-2/release/2026-09-23.1/theme=transportation/type=segment/*.parquet', hive_partitioning=false)
  WHERE in_nyc(bbox)
) TO 'data/nyc/segment.parquet' (FORMAT PARQUET, COMPRESSION ZSTD);

SELECT count(*) AS segments FROM 'data/nyc/segment.parquet';
