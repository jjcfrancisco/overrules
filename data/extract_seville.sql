-- Seville, Spain: roughly the SE-30 ring road and everything inside it.
INSTALL httpfs; LOAD httpfs;
SET s3_region='us-west-2';
SET enable_progress_bar=false;

CREATE OR REPLACE MACRO in_seville(b) AS
  b.xmin <= -5.85 AND b.xmax >= -6.05 AND b.ymin <= 37.46 AND b.ymax >= 37.30;

COPY (
  SELECT * FROM read_parquet('s3://overturemaps-us-west-2/release/2026-09-23.1/theme=transportation/type=segment/*.parquet', hive_partitioning=false)
  WHERE in_seville(bbox)
) TO 'data/seville/segment.parquet' (FORMAT PARQUET, COMPRESSION ZSTD);

COPY (
  SELECT * FROM read_parquet('s3://overturemaps-us-west-2/release/2026-09-23.1/theme=transportation/type=connector/*.parquet', hive_partitioning=false)
  WHERE in_seville(bbox)
) TO 'data/seville/connector.parquet' (FORMAT PARQUET, COMPRESSION ZSTD);

SELECT 'segments' AS t, count(*) AS n FROM 'data/seville/segment.parquet'
UNION ALL
SELECT 'connectors', count(*) FROM 'data/seville/connector.parquet';
