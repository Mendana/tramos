-- 0004: quita `runners.source_id`. Guardaba el `0x80` del .spl, que no es un id sino la longitud
-- del registro del corredor (ver docs/formato-spl.md y docs/almacenamiento.md).
ALTER TABLE runners DROP COLUMN source_id;
