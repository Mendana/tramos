-- 0010: primera llegada de cada paquete recibido (#166, docs/paquete.md "Importar").
--
-- `imported_at_epoch_ms` se actualiza cada vez que el paquete se sustituye; la nueva columna
-- guarda cuándo llegó por primera vez el par (runner_id, race_id) y no vuelve a cambiar. Las
-- novedades de quien entrena cuentan con ella: un paquete que se reexporta cambiado no es una
-- carrera nueva. Los paquetes que ya había toman como primera llegada la que tienen.
--
-- El valor por defecto solo existe porque SQLite exige uno para añadir una columna NOT NULL; la
-- aplicación siempre da la suya.
ALTER TABLE received_packages ADD COLUMN first_imported_at_epoch_ms INTEGER NOT NULL DEFAULT 0;
UPDATE received_packages SET first_imported_at_epoch_ms = imported_at_epoch_ms;
